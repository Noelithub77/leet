import gdb, os, itertools, math, re

def command(text):
    return gdb.execute(text, to_string=True)

def user(frame):
    sal=frame.find_sal()
    return sal.symtab is not None and os.path.realpath(sal.symtab.fullname())==SPEC['solution']

def encoder():
    seen={}
    cap=SPEC['max_items']
    def value(v,depth=0,label=None):
        try: return encode(v,depth,label)
        except (gdb.error, RuntimeError, ValueError, OverflowError, AttributeError, TypeError): return {'t':'opaque','type_name':str(v.type)}
    def encode(v,depth,label):
        ty=v.type.strip_typedefs(); name=str(ty)
        if ty.code in (gdb.TYPE_CODE_REF,gdb.TYPE_CODE_RVALUE_REF): return value(v.referenced_value(),depth,label)
        if ty.code==gdb.TYPE_CODE_BOOL: return {'t':'bool','v':bool(v)}
        if ty.code in (gdb.TYPE_CODE_INT,gdb.TYPE_CODE_ENUM):
            if name in ('char','signed char','unsigned char'): return {'t':'char','v':chr(int(v)&255)}
            return {'t':'int','v':str(int(v))}
        if ty.code==gdb.TYPE_CODE_FLT:
            x=float(v); return {'t':'float','v':x} if math.isfinite(x) else {'t':'opaque','type_name':name}
        if depth>=4: return {'t':'opaque','type_name':name}
        if ty.code==gdb.TYPE_CODE_PTR:
            address=int(v)
            if not address: return {'t':'none'}
            target=str(ty.target().strip_typedefs())
            if target not in ('ListNode','TreeNode'): return {'t':'opaque','type_name':name}
            ident=hex(address)
            if ident in seen: return {'t':'ref','id':seen[ident]}
            seen[ident]='n'+str(len(seen)+1)
            if target=='ListNode':
                nodes=[]; positions={}; cycle=None; node=v
                while int(node) and len(nodes)<cap:
                    addr=int(node)
                    if addr in positions: cycle=positions[addr]; break
                    positions[addr]=len(nodes); seen[hex(addr)]='linked:'+label+':'+str(len(nodes)) if label is not None else 'n'+str(len(seen)+1)
                    obj=node.dereference(); nodes.append(value(obj['val'],depth+1)); node=obj['next']
                if int(node) in positions: cycle=positions[int(node)]
                return {'t':'linked','nodes':nodes,'cycle_to':cycle}
            nodes=[]; queue=[v]; done=set()
            while queue and len(nodes)<cap:
                node=queue.pop(0); addr=int(node)
                if addr in done: continue
                done.add(addr)
                if hex(addr) not in seen: seen[hex(addr)]='n'+str(len(seen)+1)
                obj=node.dereference()
                left,right=obj['left'],obj['right']
                for child in (left,right):
                    if int(child) and hex(int(child)) not in seen: seen[hex(int(child))]='n'+str(len(seen)+1)
                nodes.append({'id':seen[hex(addr)],'value':value(obj['val'],depth+1),'left':seen[hex(int(left))] if int(left) else None,'right':seen[hex(int(right))] if int(right) else None})
                if int(left): queue.append(left)
                if int(right): queue.append(right)
            kept={n['id'] for n in nodes}
            for n in nodes:
                for side in ('left','right'):
                    if n[side] not in kept: n[side]=None
            return {'t':'tree','root':seen[ident] if nodes else None,'nodes':nodes}
        # Identity applies to aggregate storage, never to primitive values.
        if v.address is not None:
            ident=(hex(int(v.address)),name)
            if ident in seen: return {'t':'ref','id':seen[ident]}
            seen[ident]='n'+str(len(seen)+1)
        printer=gdb.default_visualizer(v)
        if printer:
            hint=printer.display_hint() if hasattr(printer,'display_hint') else None
            if hint=='string':
                s=printer.to_string()
                if hasattr(s,'value'): s=s.value().string(length=min(s.length,65536)) if s.length>=0 else s.value().string(length=65536)
                elif isinstance(s,gdb.Value): s=s.string()
                return {'t':'str','v':str(s)[:65536]}
            if hasattr(printer,'children'):
                is_map=hint=='map'; limit=cap*2 if is_map else cap
                # Known printers expose sizes cheaply; do not enumerate dropped children.
                total=printer.num_children() if hasattr(printer,'num_children') else None
                if total is not None: total=int(total)
                try:
                    if total is not None: pass
                    elif 'vector<' in name: total=int(v['_M_impl']['_M_finish']-v['_M_impl']['_M_start'])
                    elif 'unordered_' in name: total=int(v['_M_h']['_M_element_count'])*(2 if is_map else 1)
                    elif 'map<' in name or 'set<' in name: total=int(v['_M_t']['_M_impl']['_M_node_count'])*(2 if is_map else 1)
                except gdb.error: pass
                if total is not None and total<0: return {'t':'opaque','type_name':name}
                children=list(itertools.islice(printer.children(),limit+1))
                dropped=max(0,(total if total is not None else len(children))-limit)//(2 if is_map else 1)
                children=children[:limit]
                if is_map: return {'t':'map','kind':'unordered_map' if 'unordered_map<' in name else 'map','entries':[[value(children[i][1],depth+1),value(children[i+1][1],depth+1)] for i in range(0,len(children)-1,2)],'truncated':dropped}
                kind=name.split('<',1)[0].split('::')[-1]
                if kind not in ('priority_queue','unordered_set','set','deque','stack','queue','vector','array','pair'): kind='list'
                return {'t':'set' if kind in ('set','unordered_set') else 'list','kind':'heap' if kind=='priority_queue' else kind,'items':[value(x,depth+1) for _,x in children],'truncated':dropped}
        if ty.code==gdb.TYPE_CODE_ARRAY:
            lo,hi=ty.range(); items=[value(v[i],depth+1) for i in range(lo,min(hi+1,lo+cap))]
            return {'t':'list','kind':'array','items':items,'truncated':max(0,hi-lo+1-cap)}
        if ty.code==gdb.TYPE_CODE_STRUCT:
            fields=[f for f in ty.fields() if f.name and not f.is_base_class][:8]
            return {'t':'object','class':name,'fields':[[f.name,value(v[f.name],depth+1)] for f in fields]}
        return {'t':'opaque','type_name':name}
    return value

def capture(frame,kind,visited):
    encode=encoder(); sal=frame.find_sal(); stack=[]; f=frame
    while f:
        if user(f): stack.append({'function':f.name() or '?','line':f.find_sal().line})
        f=f.older()
    function=(frame.name() or '').split('::')
    constructor=len(function)>1 and function[-1]==function[-2]
    destructor=function[-1].startswith('~')
    variables=[]; names=set(); block=frame.block(); blocks=[]
    while block and not block.is_global and not block.is_static:
        blocks.append(block); block=block.superblock
    # Inner bindings take precedence; ordering follows source declarations.
    symbols=[]
    for block in blocks:
        for symbol in block:
            if not (symbol.is_argument or symbol.is_variable) or symbol.name in names: continue
            names.add(symbol.name)
            if symbol.line and symbol.line>sal.line and not symbol.is_argument: continue
            symbols.append(symbol)
    symbols.sort(key=lambda s:(0 if s.is_argument else 1,s.line))
    for symbol in symbols:
        try:
            v=frame.read_var(symbol)
            if symbol.name=='this':
                if v and str(v.type.target().strip_typedefs())!='Solution':
                    obj=v.dereference()
                    for field in obj.type.fields():
                        if field.name and not field.is_base_class: variables.append({'name':'self.'+field.name,'value':{'t':'opaque','type_name':str(field.type)} if destructor or (constructor and kind=='call') else encode(obj[field.name],0,'self.'+field.name)})
            else: variables.append({'name':symbol.name,'value':{'t':'opaque','type_name':str(v.type)} if not symbol.is_argument and symbol.line==sal.line and symbol.line not in visited else encode(v,0,symbol.name)})
        except gdb.error: continue
    stdout_len=os.path.getsize(SPEC['stdout']) if os.path.exists(SPEC['stdout']) else 0
    return {'kind':kind,'line':sal.line,'function':frame.name() or '?','stack':stack[::-1],'locals':variables,'stdout_len':stdout_len}

class TraceBudgetExceeded(RuntimeError):
    pass

def main():
    result={'truncated':False}; signals=[]
    def stopped(event):
        if isinstance(event,gdb.SignalEvent): signals.append(event.stop_signal)
    gdb.events.stop.connect(stopped)
    events=open(SPEC['events'],'w',encoding='utf-8',buffering=1); count=0; trace_bytes=0
    def write_step(step):
        nonlocal count, trace_bytes
        data=(json.dumps(step,separators=(',',':'),ensure_ascii=False)+'\n').encode('utf-8')
        if trace_bytes+len(data)>6*1048576:
            result['truncated']=True
            raise TraceBudgetExceeded('Trace exceeded 6 MiB; reduce max_items or use a smaller test case')
        events.write(data.decode('utf-8')); trace_bytes+=len(data); count+=1
    try:
        command('set pagination off'); command('set confirm off'); command('set print elements '+str(SPEC['max_items']))
        command('set debuginfod enabled off'); command('set breakpoint pending on')
        command('skip -gfi /usr/include/*'); command('skip -gfi /usr/lib/*')
        gdb.Breakpoint('main',internal=True)
        command('run < "'+SPEC['input']+'" > "'+SPEC['stdout']+'" 2> "'+SPEC['stderr']+'"')
        # Break every executable solution line. Continuing between these breakpoints
        # avoids thousands of stops inside STL operations and still sees user recursion.
        pcs=set()
        lines=open(SPEC['solution'],encoding='utf-8').read().splitlines()
        breaks=[]
        for line in range(1,len(lines)+1):
            try:
                info=command('info line '+json.dumps(SPEC['solution'],ensure_ascii=False)+':'+str(line))
                match=re.search(r'starts at address (0x[0-9a-f]+)',info)
                if match and match[1] not in pcs:
                    pcs.add(match[1]); breaks.append(gdb.Breakpoint(json.dumps(SPEC['solution'],ensure_ascii=False)+':'+str(line),internal=True))
            except gdb.error: pass
        pending=[]; active={}
        class Finish(gdb.FinishBreakpoint):
            def __init__(self,frame,key):
                super().__init__(frame,internal=True)
                self.key=key
                self.snapshot=None
                self.visited=set()
                self.instance=None
                names=(frame.name() or '').split('::')
                if len(names)>1 and names[-1]==names[-2]:
                    try:
                        self.instance=frame.read_var('this');self.instance.fetch_lazy()
                    except gdb.error: pass
            def stop(self):
                snapshot=self.snapshot
                if snapshot is not None:
                    snapshot=dict(snapshot);snapshot['kind']='return'
                    if self.instance is not None:
                        try:
                            obj=self.instance.dereference();encode=encoder()
                            snapshot['locals']=[v for v in snapshot['locals'] if not v['name'].startswith('self.')]
                            for field in obj.type.fields():
                                if field.name and not field.is_base_class: snapshot['locals'].append({'name':'self.'+field.name,'value':encode(obj[field.name],0,'self.'+field.name)})
                        except gdb.error: pass
                    snapshot['returned']=encoder()(self.return_value) if self.return_value is not None else {'t':'none'}
                    pending.append(snapshot)
                active.pop(self.key,None)
                return True
            def out_of_scope(self):
                active.pop(self.key,None)
        while gdb.selected_inferior().threads():
            if signals:
                result['error']='Program stopped with '+signals[-1]
                if count<SPEC['max_steps']:
                    frame=gdb.newest_frame()
                    if user(frame):
                        step=capture(frame,'exception',set());step['exception']=signals[-1]
                        write_step(step)
                command('kill');break
            for step in pending:
                if count>=SPEC['max_steps']: break
                write_step(step)
            pending.clear()
            if count>=SPEC['max_steps']:
                result['truncated']=True; command('kill'); break
            frame=gdb.newest_frame()
            if user(frame):
                key=(frame.name(),int(frame.read_register('sp')))
                new=key not in active
                if new:
                    try: active[key]=Finish(frame,key)
                    except gdb.error: active[key]=None
                step=capture(frame,'call' if new else 'line',active[key].visited if active[key] is not None else set())
                if active[key] is not None:
                    active[key].snapshot=step; active[key].visited.add(step['line'])
                write_step(step)
            command('continue')
        for step in pending:
            if count>=SPEC['max_steps']: result['truncated']=True; break
            write_step(step)
    except (gdb.error,RuntimeError,ValueError,OverflowError,AttributeError,TypeError) as e:
        result['error']=str(e)
        try:
            if gdb.selected_inferior().threads(): command('kill')
        except gdb.error: pass
    finally:
        gdb.events.stop.disconnect(stopped)
        events.close()
        data=json.dumps(result).encode('utf-8')
        if len(data)>2*1048576:
            data=json.dumps({'truncated':True,'error':'Trace result exceeded 2 MiB; use a smaller test case'}).encode('utf-8')
        with open(SPEC['result'],'wb') as f: f.write(data)
main()
