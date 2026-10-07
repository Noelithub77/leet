# Appended to the existing harness definitions; user code runs in its own module.
import types

class _Output(io.StringIO):
    def __init__(self, path):
        super().__init__()
        self.file = open(path, 'w', encoding='utf-8', buffering=1)
        self.byte_len = 0
    def write(self, text):
        if self.byte_len+len(text.encode('utf-8'))>1048576: raise RuntimeError('Program stdout exceeded 1 MiB')
        self.file.write(text)
        self.byte_len += len(text.encode('utf-8'))
        self.file.flush()
        return super().write(text)

class _TraceStop(BaseException):
    pass

def _attrs(value):
    # Only builtin storage descriptors: never invoke user properties/getattr.
    cls = type(value)
    fields = {}
    for base in reversed(type.__getattribute__(cls, '__mro__')):
        namespace = type.__getattribute__(base, '__dict__')
        descriptor = namespace.get('__dict__')
        if isinstance(descriptor, types.GetSetDescriptorType):
            try:
                raw = descriptor.__get__(value, cls)
                if type(raw) is dict: fields.update(raw)
            except (AttributeError, TypeError): pass
        for name, descriptor in namespace.items():
            if isinstance(descriptor, types.MemberDescriptorType):
                try: fields[name] = descriptor.__get__(value, cls)
                except AttributeError: pass
    return fields

def _exception(exc):
    args = BaseException.args.__get__(exc, type(exc))
    parts = [x[:300] if type(x) is str else json.dumps(x) for x in args[:3] if type(x) in (str, bool, int, float) or x is None]
    return type(exc).__name__ + (': ' + ', '.join(parts) if parts else '')

def _record_main():
    spec = json.load(sys.stdin)
    path, cap = spec['path'], spec['max_items']
    stdout = _Output(spec['stdout_file'])
    result = {'truncated': False}
    count = 0
    trace_bytes = 0
    events = open(spec['events'], 'w', encoding='utf-8', buffering=1)
    module = {k: v for k, v in globals().items() if not k.startswith('__')}
    module.update(__name__='solution', __file__=path)

    def encoder():
        seen = {}
        def value(v, depth=0, label=None):
            cls = type(v)
            if v is None: return {'t':'none'}
            if cls is bool: return {'t':'bool','v':v}
            if cls is int: return {'t':'int','v':str(v)}
            if cls is float: return {'t':'float','v':v} if math.isfinite(v) else {'t':'opaque','type_name':'float'}
            if cls is str: return {'t':'str','v':v[:65536]}
            ident = id(v)
            if ident in seen: return {'t':'ref','id':seen[ident]}
            if depth >= 4: return {'t':'opaque','type_name':cls.__name__}
            seen[ident] = 'n'+str(len(seen)+1)
            if cls in (list, tuple, collections.deque, set, frozenset):
                # Builtin iteration never invokes element repr/equality.
                items = [value(x,depth+1) for x in itertools.islice(v,cap)]
                if cls in (set, frozenset): items.sort(key=lambda x: json.dumps(x, sort_keys=True))
                return {'t':'set' if cls in (set,frozenset) else 'list','kind':cls.__name__,'items':items,'truncated':max(0,len(v)-cap)}
            if cls in (dict, collections.defaultdict, collections.Counter, collections.OrderedDict):
                return {'t':'map','kind':cls.__name__,'entries':[[value(k,depth+1),value(x,depth+1)] for k,x in itertools.islice(v.items(),cap)],'truncated':max(0,len(v)-cap)}
            fields = _attrs(v)
            if 'val' in fields and 'left' in fields and 'right' in fields:
                nodes, queue, done = [], collections.deque([v]), set()
                while queue and len(nodes)<cap:
                    node = queue.popleft()
                    if id(node) in done: continue
                    done.add(id(node)); f = _attrs(node)
                    children = []
                    for side in ('left','right'):
                        child = f.get(side)
                        if child is None: children.append(None); continue
                        if id(child) not in seen: seen[id(child)]='n'+str(len(seen)+1)
                        children.append(seen[id(child)]); queue.append(child)
                    nodes.append({'id':seen[id(node)],'value':value(f.get('val'),depth+1),'left':children[0],'right':children[1]})
                kept = {n['id'] for n in nodes}
                for n in nodes:
                    for side in ('left','right'):
                        if n[side] not in kept: n[side]=None
                return {'t':'tree','root':seen[ident] if nodes else None,'nodes':nodes}
            if 'val' in fields and 'next' in fields:
                nodes, positions, node, cycle = [], {}, v, None
                while node is not None and len(nodes)<cap:
                    if id(node) in positions: cycle=positions[id(node)]; break
                    positions[id(node)]=len(nodes)
                    if label is not None: seen[id(node)]='linked:'+label+':'+str(len(nodes))
                    elif id(node) not in seen: seen[id(node)]='n'+str(len(seen)+1)
                    f=_attrs(node); nodes.append(value(f.get('val'),depth+1)); node=f.get('next')
                if node is not None and id(node) in positions: cycle=positions[id(node)]
                return {'t':'linked','nodes':nodes,'cycle_to':cycle}
            return {'t':'object','class':cls.__name__,'fields':[[k,value(x,depth+1)] for k,x in itertools.islice(((k,x) for k,x in fields.items() if isinstance(k,str) and not k.startswith('_')),8)]}
        return value

    def trace(frame, event, arg):
        nonlocal count, trace_bytes
        if frame.f_code.co_filename != path: return trace
        # Comprehension and generator frames are compiler machinery, not named user functions.
        if frame.f_code.co_name.startswith('<') and frame.f_code.co_name != '<lambda>': return trace
        if event not in ('call','line','return','exception'): return trace
        if count >= spec['max_steps']:
            result['truncated']=True; sys.settrace(None); raise _TraceStop()
        encode_value = encoder()
        variables=[]
        for name,v in frame.f_locals.items():
            if not name.isidentifier() or name == 'self' or name.startswith('__') or isinstance(v,(types.ModuleType,types.FunctionType,types.MethodType,type)): continue
            variables.append({'name':name,'value':encode_value(v,0,name)})
        obj=frame.f_locals.get('self')
        if obj is not None and type(obj).__name__ != 'Solution':
            for name,v in _attrs(obj).items():
                if not name.startswith('__') and not isinstance(v,(types.ModuleType,types.FunctionType,types.MethodType,type)): variables.append({'name':'self.'+name,'value':encode_value(v,0,'self.'+name)})
        stack=[]; current=frame
        while current:
            if current.f_code.co_filename == path and (not current.f_code.co_name.startswith('<') or current.f_code.co_name == '<lambda>'): stack.append({'function':current.f_code.co_name.strip('<>'),'line':current.f_lineno})
            current=current.f_back
        step={'kind':event,'line':frame.f_lineno,'function':frame.f_code.co_name.strip('<>'),'stack':stack[::-1],'locals':variables,'stdout_len':stdout.byte_len}
        if event=='return': step['returned']=encode_value(arg)
        if event=='exception': step['exception']=_exception(arg[1])
        data = (json.dumps(step,separators=(',',':'),ensure_ascii=False)+'\n').encode('utf-8')
        if trace_bytes + len(data) > 6 * 1048576:
            result.update(truncated=True, error='Trace exceeded 6 MiB; reduce max_items or use a smaller test case')
            sys.settrace(None); raise _TraceStop()
        events.write(data.decode('utf-8')); trace_bytes += len(data); count += 1
        return trace
    try:
        with redirect_stdout(stdout):
            exec(compile(open(path,encoding='utf-8').read(),path,'exec'),module)
            sys.settrace(trace)
            try:
                meta=spec['meta']
                value=(run_design if meta.get('systemdesign') or 'classname' in meta else run_function)(module,meta,spec['input'])
            finally: sys.settrace(None)
            result['output']=dump(value)
    except _TraceStop: pass
    except BaseException as exc:
        tb = BaseException.__traceback__.__get__(exc, type(exc))
        result['error']=(''.join(traceback.format_list(traceback.extract_tb(tb, limit=-4)))+_exception(exc))[-4000:]
    finally:
        sys.settrace(None); events.close(); result['stdout']=stdout.getvalue(); stdout.file.close()
        data = json.dumps(result,ensure_ascii=False).encode('utf-8')
        if len(data) > 2 * 1048576:
            result.update(truncated=True, error='Trace result exceeded 2 MiB; use a smaller test case')
            result.pop('output', None); result['stdout'] = result['stdout'][:65536]
            data = json.dumps(result,ensure_ascii=False).encode('utf-8')
        with open(spec['result'],'wb') as f: f.write(data)

_record_main()
