use super::{Step, StepKind, Value};
use crate::viz::*;
fn scalar(v: &Value) -> bool { matches!(v,Value::None|Value::Bool{..}|Value::Int{..}|Value::Float{..}|Value::Str{..}|Value::Char{..}) }
fn text(v: &Value) -> String {
    match v {
        Value::None => "null".into(), Value::Bool{v} => v.to_string(), Value::Int{v}|Value::Str{v}|Value::Char{v} => v.clone(), Value::Float{v} => v.to_string(),
        Value::Ref{id} => {
            if let Some((name,index))=id.strip_prefix("linked:").and_then(|s|s.rsplit_once(':')) {format!("→{name}[{index}]")}else{format!("→{id}")}
        }, Value::Opaque{type_name} => format!("<{type_name}>"), Value::Object{class,..} => class.clone(),
        Value::List{items,..}|Value::Set{items,..}|Value::Linked{nodes:items,..} => format!("[{}]",items.iter().map(text).collect::<Vec<_>>().join(", ")),
        Value::Map{entries,..} => format!("{{{}}}",entries.iter().map(|(k,v)|format!("{}: {}",text(k),text(v))).collect::<Vec<_>>().join(", ")),
        Value::Tree{nodes,..} => format!("tree ({})",nodes.len()),
    }
}
fn index(v: &Value) -> Option<usize> { if let Value::Int{v}=v { v.parse().ok() } else { None } }
fn index_name(name: &str) -> bool {
    let name=name.rsplit('.').next().unwrap_or(name).to_lowercase(); matches!(name.as_str(),"i"|"j"|"k"|"l"|"r"|"lo"|"hi"|"left"|"right"|"mid"|"start"|"end"|"slow"|"fast"|"p"|"q"|"idx"|"index"|"ptr"|"top") || name.ends_with("_idx") || name.ends_with("_index")
}
fn tone(current: &Value, old: Option<&Value>) -> Tone { if old.is_some_and(|v|v!=current) { Tone::Changed } else { Tone::Default } }
fn cells(items: &[Value], old: Option<&Value>) -> Vec<Cell> {
    let old=match old { Some(Value::List{items,..}|Value::Set{items,..}|Value::Linked{nodes:items,..})=>Some(items),_=>None };
    items.iter().enumerate().map(|(i,v)|Cell{value:text(v),tone:if old.is_some() && old.and_then(|a|a.get(i))!=Some(v) {Tone::Changed} else {Tone::Default}}).collect()
}
fn list(v: &Value) -> Option<&[Value]> { if let Value::List{items,..}=v {Some(items)} else {None} }
fn window(pointers:&[Pointer])->Vec<Span> {
    let left=pointers.iter().find(|p|matches!(p.name.rsplit('.').next().unwrap_or(&p.name).to_lowercase().as_str(),"l"|"lo"|"left"|"start"));
    let right=pointers.iter().find(|p|matches!(p.name.rsplit('.').next().unwrap_or(&p.name).to_lowercase().as_str(),"r"|"hi"|"right"|"end"));
    left.zip(right).filter(|(l,r)|l.index<=r.index).map(|(l,r)|vec![Span{from:l.index,to:r.index,label:"window".into(),tone:Tone::Active}]).unwrap_or_default()
}
fn number(v: &Value) -> Option<f64> {match v {Value::Int{v}=>v.parse().ok(),Value::Float{v}=>Some(*v),_=>None}}
/// Index markers attach only to a sole in-bounds sequence, or a sequence named by
/// `nums_idx` / `nums_index`. With several compatible arrays we leave them off.
/// Recorder refs use `linked:<variable>:<index>` for chain nodes. Other linked
/// locals use value matches only when unique; tree refs match node ids exactly.
pub fn structures(step: &Step, previous: Option<&Step>) -> Vec<Structure> {
    let old=|name:&str| previous.and_then(|s|s.locals.iter().find(|v|v.name==name).map(|v|&v.value));
    let sequence_len=|v:&Value| match v {Value::List{items,..} if items.iter().all(scalar)=>Some(items.len()),Value::Str{v}=>Some(v.chars().count()),_=>None};
    let pointers=|name:&str,len:usize| -> Vec<Pointer> {
        step.locals.iter().filter(|v|index_name(&v.name)).filter_map(|v| {
            let i=index(&v.value)?; if i>=len {return None;}
            let index_name=v.name.to_lowercase();
            let explicit=index_name.strip_suffix("_idx").or_else(||index_name.strip_suffix("_index"));
            let candidates=step.locals.iter().filter(|v|sequence_len(&v.value).is_some_and(|n|i<n)).count();
            if explicit==Some(name.to_lowercase().as_str()) || (explicit.is_none() && candidates==1) {Some(Pointer{name:v.name.clone(),index:i,tone:Tone::Active})} else {None}
        }).collect()
    };
    let mut vars=Vec::new(); let mut shapes=Vec::new();
    for variable in &step.locals {
        let name=&variable.name; let value=&variable.value; let label=name.clone(); let before=old(name); let lower=name.rsplit('.').next().unwrap_or(name).to_lowercase();
        match value {
            Value::List{kind,items,..} if items.iter().all(scalar) => {
                let mut items=cells(items,before);
                if matches!(lower.as_str(),"stack"|"stk"|"st") || kind=="stack" {shapes.push(Structure::Stack{label,items});}
                else if matches!(lower.as_str(),"queue"|"q"|"bfs") || matches!(kind.as_str(),"deque"|"queue") {shapes.push(Structure::Queue{label,items});}
                else if matches!(lower.as_str(),"heap"|"pq"|"minheap"|"maxheap"|"h") || kind=="heap" {shapes.push(Structure::Heap{label,items,min:!lower.starts_with("max")});}
                else {
                    let pointers=pointers(name,items.len()); for p in &pointers {if items[p.index].tone!=Tone::Changed {items[p.index].tone=Tone::Active;}}
                    let spans=window(&pointers);
                    shapes.push(Structure::Array{label,items,pointers,spans});
                }
            }
            Value::List{items,..} if items.iter().all(|v|list(v).is_some_and(|a|a.iter().all(scalar))) => {
                let rows=items.iter().filter_map(|v|list(v)).collect::<Vec<_>>();
                if matches!(lower.as_str(),"intervals"|"meetings"|"ranges") && rows.iter().all(|r|r.len()==2 && r.iter().all(|v|number(v).is_some())) {
                    let intervals=items.iter().enumerate().map(|(i,v)| {let row=list(v).unwrap_or_default();Interval{start:number(&row[0]).unwrap_or_default(),end:number(&row[1]).unwrap_or_default(),label:i.to_string(),tone:if before.is_some() && before.and_then(list).and_then(|a|a.get(i))!=Some(v) {Tone::Changed}else{Tone::Default}}}).collect();
                    shapes.push(Structure::Intervals{label,items:intervals});
                } else {
                    let mut rows=items.iter().enumerate().map(|(i,v)|cells(list(v).unwrap_or_default(),before.and_then(list).and_then(|a|a.get(i)))).collect::<Vec<_>>();
                    let mut pointers=Vec::new();
                    for (r,c) in [("r","c"),("i","j"),("row","col")] {
                        let find=|name:&str|step.locals.iter().find(|v|v.name.eq_ignore_ascii_case(name)).and_then(|v|index(&v.value));
                        if let Some((row,col))=find(r).zip(find(c)).filter(|&(row,col)|rows.get(row).is_some_and(|r|col<r.len())) {
                            pointers.push(GridPointer{name:format!("{r},{c}"),row,col,tone:Tone::Active});if rows[row][col].tone!=Tone::Changed{rows[row][col].tone=Tone::Active;}
                        }
                    }
                    let cols=rows.iter().map(Vec::len).max().unwrap_or(0);
                    shapes.push(Structure::Grid{label,row_labels:(0..rows.len()).map(|i|i.to_string()).collect(),col_labels:(0..cols).map(|i|i.to_string()).collect(),rows,pointers});
                }
            }
            Value::Str{v} if !pointers(name,v.chars().count()).is_empty() => {
                let chars=v.chars().map(|c|Value::Char{v:c.to_string()}).collect::<Vec<_>>();
                let old_chars=if let Some(Value::Str{v})=before {Some(Value::List{kind:"string".into(),items:v.chars().map(|c|Value::Char{v:c.to_string()}).collect(),truncated:0})}else{None};
                let mut items=cells(&chars,old_chars.as_ref());let pointers=pointers(name,items.len());for p in &pointers{if items[p.index].tone!=Tone::Changed{items[p.index].tone=Tone::Active;}}
                let spans=window(&pointers);shapes.push(Structure::Array{label,items,pointers,spans});
            }
            Value::Set{items,..} => {
                let items=items.iter().map(|v|Cell{value:text(v),tone:if let Some(Value::Set{items,..})=before {if items.contains(v){Tone::Default}else{Tone::Changed}}else{Tone::Default}}).collect();
                shapes.push(Structure::Set{label,items});
            }
            Value::Map{entries,..} => {
                let named_graph=matches!(lower.as_str(),"graph"|"adj"|"g"|"edges");
                let adjacency=!entries.is_empty() && entries.iter().all(|(k,v)| (if named_graph{scalar(k)}else{matches!(k,Value::Int{..}|Value::Str{..})}) && list(v).is_some_and(|a|a.iter().all(|v|if named_graph{scalar(v)}else{matches!(v,Value::Int{..}|Value::Str{..})})));
                if adjacency {
                    let mut nodes=Vec::new();let mut edges=Vec::new();
                    for (k,v) in entries {
                        let from=text(k);
                        let prior=if let Some(Value::Map{entries,..})=before {entries.iter().find(|(key,_)|key==k).map(|(_,v)|v)}else{None};
                        let changed=before.is_some() && prior!=Some(v);
                        if !nodes.iter().any(|n:&GraphNode|n.id==from){nodes.push(GraphNode{id:from.clone(),label:from.clone(),tone:if changed{Tone::Changed}else{Tone::Default}});}
                        for target in list(v).unwrap_or_default(){let to=text(target);if !nodes.iter().any(|n|n.id==to){nodes.push(GraphNode{id:to.clone(),label:to.clone(),tone:Tone::Default});}edges.push(Edge{from:from.clone(),to,label:String::new(),tone:if before.is_some() && !prior.and_then(list).is_some_and(|items|items.contains(target)){Tone::Changed}else{Tone::Default}});}
                    }
                    shapes.push(Structure::Graph{label,nodes,edges,directed:true});
                } else {
                    let entries=entries.iter().map(|(k,v)| {let was=if let Some(Value::Map{entries,..})=before {entries.iter().find(|(key,_)|key==k).map(|(_,v)|v)}else{None};Entry{key:text(k),value:text(v),tone:if before.is_some()&&was!=Some(v){Tone::Changed}else{Tone::Default}}}).collect();
                    shapes.push(Structure::Map{label,entries});
                }
            }
            Value::Tree{root,nodes} => {
                let nodes=nodes.iter().map(|n| {let was=if let Some(Value::Tree{nodes,..})=before {nodes.iter().find(|old|old.id==n.id)}else{None};let active=step.locals.iter().any(|v|matches!(&v.value,Value::Ref{id} if id==&n.id));TreeNode{id:n.id.clone(),value:text(&n.value),left:n.left.clone(),right:n.right.clone(),tone:if before.is_some()&&was!=Some(n){Tone::Changed}else if active{Tone::Active}else{Tone::Default}}}).collect();
                shapes.push(Structure::Tree{label,root:root.clone(),nodes});
            }
            Value::Linked{nodes,cycle_to} => {
                let mut cells=cells(nodes,before);let mut pointers=Vec::new();
                for v in &step.locals {
                    if v.name==*name {continue;}
                    if let Value::Ref{id}=&v.value {
                        if let Some(index)=id.strip_prefix(&format!("linked:{name}:")).and_then(|i|i.parse::<usize>().ok()).filter(|&i|i<nodes.len()) {
                            pointers.push(Pointer{name:v.name.clone(),index,tone:Tone::Active});if cells[index].tone!=Tone::Changed{cells[index].tone=Tone::Active;}
                        }
                        continue;
                    }
                    let target=match &v.value {Value::Linked{nodes,..}=>nodes.first(),value if scalar(value) && index_name(&v.name)=>Some(value),_=>None};
                    if let Some(target)=target {let hits=nodes.iter().enumerate().filter(|(_,n)|*n==target).map(|(i,_)|i).collect::<Vec<_>>();if hits.len()==1{let index=hits[0];pointers.push(Pointer{name:v.name.clone(),index,tone:Tone::Active});if cells[index].tone!=Tone::Changed{cells[index].tone=Tone::Active;}}}
                }
                shapes.push(Structure::LinkedList{label,nodes:cells,pointers,cycle_to:*cycle_to});
            }
            _ => vars.push(Var{name:label,value:text(value),tone:tone(value,before)}),
        }
    }
    if step.kind==StepKind::Return && let Some(value)=&step.returned {vars.push(Var{name:"return".into(),value:text(value),tone:Tone::Active});}
    if !vars.is_empty(){shapes.insert(0,Structure::Vars{label:"vars".into(),vars});} shapes
}
#[cfg(test)] mod tests {
    use super::*;
    fn step(locals:serde_json::Value)->Step{serde_json::from_value(serde_json::json!({"kind":"line","line":1,"function":"f","stack":[],"locals":locals})).unwrap()}
    #[test] fn debugger_shapes_pointers_and_changes(){
        let before=step(serde_json::json!([{"name":"nums","value":{"t":"list","kind":"list","items":[{"t":"int","v":"1"},{"t":"int","v":"2"}]}},{"name":"i","value":{"t":"int","v":"0"}}]));
        let mut current=before.clone();current.locals[1].value=Value::Int{v:"1".into()};
        let shapes=structures(&current,Some(&before));
        assert!(matches!(&shapes[0],Structure::Vars{vars,..} if vars[0].tone==Tone::Changed));
        assert!(matches!(&shapes[1],Structure::Array{items,pointers,..} if items[1].tone==Tone::Active && pointers[0].index==1));
        current.locals.push(current.locals[0].clone());current.locals[2].name="other".into();
        assert!(matches!(&structures(&current,None)[1],Structure::Array{pointers,..} if pointers.is_empty()));
    }
    #[test] fn debugger_special_shapes(){
        let values=serde_json::json!([
            {"name":"stack","value":{"t":"list","kind":"list","items":[]}},
            {"name":"queue","value":{"t":"list","kind":"deque","items":[]}},
            {"name":"maxheap","value":{"t":"list","kind":"list","items":[]}},
            {"name":"grid","value":{"t":"list","kind":"list","items":[{"t":"list","kind":"list","items":[{"t":"int","v":"1"}]}]}},
            {"name":"ranges","value":{"t":"list","kind":"list","items":[{"t":"list","kind":"list","items":[{"t":"int","v":"1"},{"t":"int","v":"2"}]}]}},
            {"name":"adj","value":{"t":"map","kind":"dict","entries":[[{"t":"int","v":"1"},{"t":"list","kind":"list","items":[{"t":"int","v":"2"}]}]]}},
            {"name":"map","value":{"t":"map","kind":"dict","entries":[]}},
            {"name":"set","value":{"t":"set","kind":"set","items":[]}},
            {"name":"tree","value":{"t":"tree","root":null,"nodes":[]}},
            {"name":"linked","value":{"t":"linked","nodes":[],"cycle_to":null}}
        ]);
        let shapes=structures(&step(values),None);
        assert!(matches!(shapes[0],Structure::Stack{..}));assert!(matches!(shapes[1],Structure::Queue{..}));assert!(matches!(shapes[2],Structure::Heap{min:false,..}));assert!(matches!(shapes[3],Structure::Grid{..}));assert!(matches!(shapes[4],Structure::Intervals{..}));assert!(matches!(shapes[5],Structure::Graph{..}));assert!(matches!(shapes[6],Structure::Map{..}));assert!(matches!(shapes[7],Structure::Set{..}));assert!(matches!(shapes[8],Structure::Tree{..}));assert!(matches!(shapes[9],Structure::LinkedList{..}));
    }
}

#[cfg(test)] mod regression_tests {
    use super::*;
    fn step(locals:serde_json::Value)->Step{serde_json::from_value(serde_json::json!({"kind":"line","line":1,"function":"f","stack":[],"locals":locals})).unwrap()}
    #[test] fn debugger_structure_grid_linked_tree_and_window_markers() {
        let current=step(serde_json::json!([
            {"name":"grid","value":{"t":"list","kind":"list","items":[{"t":"list","kind":"list","items":[{"t":"int","v":"1"},{"t":"int","v":"2"}]}]}},
            {"name":"r","value":{"t":"int","v":"0"}}, {"name":"c","value":{"t":"int","v":"1"}},
            {"name":"head","value":{"t":"linked","nodes":[{"t":"int","v":"5"},{"t":"int","v":"5"}],"cycle_to":0}},
            {"name":"fast","value":{"t":"ref","id":"linked:head:1"}},
            {"name":"root","value":{"t":"tree","root":"n1","nodes":[{"id":"n1","value":{"t":"int","v":"1"},"left":null,"right":null}]}},
            {"name":"node","value":{"t":"ref","id":"n1"}}
        ]));
        let shapes=structures(&current,None);
        assert!(matches!(&shapes[1],Structure::Grid{rows,pointers,..} if rows[0][1].tone==Tone::Active && pointers[0].col==1));
        assert!(matches!(&shapes[2],Structure::LinkedList{nodes,pointers,cycle_to:Some(0),..} if nodes[1].tone==Tone::Active && pointers[0].index==1));
        assert!(matches!(&shapes[3],Structure::Tree{nodes,..} if nodes[0].tone==Tone::Active));
        let window=step(serde_json::json!([{"name":"word","value":{"t":"str","v":"abc"}},{"name":"left","value":{"t":"int","v":"0"}},{"name":"right","value":{"t":"int","v":"2"}}]));
        let shapes=structures(&window,None);assert!(matches!(&shapes[1],Structure::Array{items,pointers,..} if items.len()==3 && pointers.len()==2));
        let mut array=window;array.locals[0].value=Value::List{kind:"list".into(),items:vec![Value::Int{v:"1".into()};3],truncated:0};
        assert!(matches!(&structures(&array,None)[1],Structure::Array{spans,..} if spans[0].from==0 && spans[0].to==2));
    }
    #[test] fn debugger_structure_changed_cells_entries_and_return() {
        let before=step(serde_json::json!([
            {"name":"grid","value":{"t":"list","kind":"list","items":[{"t":"list","kind":"list","items":[{"t":"int","v":"1"}]}]}},
            {"name":"seen","value":{"t":"set","kind":"set","items":[{"t":"int","v":"2"}]}},
            {"name":"counts","value":{"t":"map","kind":"dict","entries":[[{"t":"str","v":"x"},{"t":"int","v":"1"}]]}}
        ]));
        let mut current=before.clone();
        if let Value::List{items,..}=&mut current.locals[0].value && let Value::List{items,..}=&mut items[0]{items[0]=Value::Int{v:"9".into()};}
        if let Value::Set{items,..}=&mut current.locals[1].value{items.insert(0,Value::Int{v:"1".into()});}
        if let Value::Map{entries,..}=&mut current.locals[2].value{entries[0].1=Value::Int{v:"2".into()};}
        current.kind=StepKind::Return;current.returned=Some(Value::Bool{v:true});
        let shapes=structures(&current,Some(&before));
        assert!(matches!(&shapes[0],Structure::Vars{vars,..} if vars[0].name=="return"));
        assert!(matches!(&shapes[1],Structure::Grid{rows,..} if rows[0][0].tone==Tone::Changed));
        assert!(matches!(&shapes[2],Structure::Set{items,..} if items[0].tone==Tone::Changed && items[1].tone==Tone::Default));
        assert!(matches!(&shapes[3],Structure::Map{entries,..} if entries[0].tone==Tone::Changed));
    }
}
