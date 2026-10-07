use super::*;
fn cases() -> Vec<(&'static str,&'static str,&'static str,&'static str,&'static str)> { vec![
(r###"{"name": "twoSum", "params": [{"name": "a0", "type": "integer[]"}, {"name": "a1", "type": "integer"}], "return": {"type": "integer[]"}}"###,r###"class Solution {
public:
 vector<int> twoSum(vector<int>& nums, int target) {
  unordered_map<int,int> seen;
  for(int i=0;i<(int)nums.size();++i) {
   int n=nums[i];
   if(seen.count(target-n)) return {seen[target-n],i};
   seen[n]=i;
  }
  return {};
 }
};"###,r###"class Solution:
 def twoSum(self, nums, target):
  seen = {}
  for i, n in enumerate(nums):
   if target-n in seen: return [seen[target-n], i]
   seen[n] = i
"###,r###"[2,7,11,15]
9"###,r###"[0,1]"###),
(r###"{"name": "reverseList", "params": [{"name": "a0", "type": "ListNode"}], "return": {"type": "ListNode"}}"###,r###"class Solution {
public:
 ListNode* reverseList(ListNode* head) {
  ListNode* prev=nullptr;
  while(head) {
   ListNode* nxt=head->next;
   head->next=prev;
   prev=head;
   head=nxt;
  }
  return prev;
 }
};"###,r###"class Solution:
 def reverseList(self, head):
  prev = None
  while head:
   nxt = head.next
   head.next = prev
   prev = head
   head = nxt
  return prev
"###,r###"[1,2,3]"###,r###"[3,2,1]"###),
(r###"{"name": "maxDepth", "params": [{"name": "a0", "type": "TreeNode"}], "return": {"type": "integer"}}"###,r###"class Solution {
public:
 int maxDepth(TreeNode* root) {
  if(!root) return 0;
  return 1+max(maxDepth(root->left),maxDepth(root->right));
 }
};"###,r###"class Solution:
 def maxDepth(self, root):
  if not root: return 0
  return 1 + max(self.maxDepth(root.left), self.maxDepth(root.right))
"###,r###"[3,9,20,null,null,15,7]"###,r###"3"###),
(r###"{"name": "groupAnagrams", "params": [{"name": "a0", "type": "string[]"}], "return": {"type": "string[][]"}}"###,r###"class Solution {
public:
 vector<vector<string>> groupAnagrams(vector<string>& strs) {
  map<string,vector<string>> groups;
  for(auto s:strs) {
   auto key=s;sort(key.begin(),key.end());groups[key].push_back(s);
  }
  vector<vector<string>> result;
  for(auto &p:groups) result.push_back(p.second);
  return result;
 }
};"###,r###"class Solution:
 def groupAnagrams(self, strs):
  groups=defaultdict(list)
  for s in strs: groups[''.join(sorted(s))].append(s)
  return list(groups.values())
"###,r###"["eat","tea","tan","ate","nat","bat"]"###,r###"[["bat"],["eat","tea","ate"],["tan","nat"]]"###),
(r###"{"name": "numIslands", "params": [{"name": "a0", "type": "character[][]"}], "return": {"type": "integer"}}"###,r###"class Solution {
public:
 void visit(vector<vector<char>>& grid,int r,int c) {
  if(r<0||c<0||r>=(int)grid.size()||c>=(int)grid[0].size()||grid[r][c]!='1') return;
  grid[r][c]='0';
  visit(grid,r+1,c);visit(grid,r-1,c);visit(grid,r,c+1);visit(grid,r,c-1);
 }
 int numIslands(vector<vector<char>>& grid) {
  int count=0;
  for(int r=0;r<(int)grid.size();++r) {
   for(int c=0;c<(int)grid[0].size();++c) {
    if(grid[r][c]=='1') {++count;visit(grid,r,c);}
   }
  }
  return count;
 }
};"###,r###"class Solution:
 def numIslands(self, grid):
  def visit(r,c):
   if r<0 or c<0 or r>=len(grid) or c>=len(grid[0]) or grid[r][c]!='1': return
   grid[r][c]='0'
   for dr,dc in [(1,0),(-1,0),(0,1),(0,-1)]: visit(r+dr,c+dc)
  count=0
  for r in range(len(grid)):
   for c in range(len(grid[0])):
    if grid[r][c]=='1':
     count+=1
     visit(r,c)
  return count
"###,r###"[["1","1","0"],["0","1","0"],["0","0","1"]]"###,r###"2"###),
(r###"{"classname": "MinStack", "systemdesign": true, "constructor": {"params": []}, "methods": [{"name": "push", "params": [{"type": "integer"}], "return": {"type": "void"}}, {"name": "pop", "params": [], "return": {"type": "void"}}, {"name": "top", "params": [], "return": {"type": "integer"}}, {"name": "getMin", "params": [], "return": {"type": "integer"}}]}"###,r###"class MinStack {
public:
 vector<int> stack;
 MinStack() {}
 void push(int x) {
  stack.push_back(x);
 }
 void pop() {
  stack.pop_back();
 }
 int top() {
  return stack.back();
 }
 int getMin() {
  return *min_element(stack.begin(),stack.end());
 }
};"###,r###"class MinStack:
 def __init__(self):
  self.stack=[]
 def push(self,x):
  self.stack.append(x)
 def pop(self):
  self.stack.pop()
 def top(self):
  return self.stack[-1]
 def getMin(self):
  return min(self.stack)
"###,r###"["MinStack","push","push","push","getMin","pop","top","getMin"]
[[],[-2],[0],[-3],[],[],[],[]]"###,r###"[null,null,null,null,-3,null,0,-2]"###),
 ] }
#[test] fn debugger_cpp_compiles_and_runs_judge_shapes() {
 if Command::new("g++").arg("--version").output().is_err() {eprintln!("Skipping C++ driver: g++ missing");return;}
 let dir=Scratch::new().unwrap();
 for (meta,code,_,input,expected) in cases() {
  let meta=serde_json::from_str(meta).unwrap();let path=dir.0.join("solution.cpp");let source=dir.0.join("main.cpp");let binary=dir.0.join("solution");
  fs::write(&source,cpp_driver::source(&meta,&path,code).unwrap()).unwrap();
  let mut compiler=Command::new("g++");compiler.args(["-std=c++20","-O0","-o"]).arg(&binary).arg(source);
  let (ok,_,_,error)=execute(&mut compiler,"",Duration::from_secs(60),&dir.0).unwrap();assert!(ok,"{error}");
  let (ok,_,output,error)=execute(&mut Command::new(binary),&format!("{input}\n"),Duration::from_secs(3),&dir.0).unwrap();assert!(ok,"{error}");assert_eq!(output.split_once("__LEET_TRACE_RESULT_91b6__\n").unwrap().1.trim(),expected);
 }
}
#[test] fn debugger_cpp_records_judge_shapes() {
 if available().is_err() {eprintln!("Skipping C++ recorder: g++/Python-enabled gdb missing");return;}
 let dir=Scratch::new().unwrap();
 for (meta,code,_,input,expected) in cases() {
  let meta=serde_json::from_str(meta).unwrap();let path=dir.0.join("solution.cpp");fs::write(&path,code).unwrap();
  let case=crate::runner::Case{id:0,input:input.into(),expected:None,custom:false};
  let trace=record(&path,&meta,&case,Limits::default()).unwrap();assert_eq!(trace.error,None);assert_eq!(trace.output.as_deref(),Some(expected));assert!(!trace.steps.is_empty());assert!(trace.steps.iter().any(|s|s.kind==super::super::StepKind::Return));
  if meta["classname"]=="MinStack" {
      let constructor=trace.steps.iter().find(|s|s.kind==super::super::StepKind::Return && s.function=="MinStack::MinStack").unwrap();
      assert!(constructor.locals.iter().any(|v|v.name=="self.stack" && matches!(&v.value,super::super::Value::List{items,..} if items.is_empty())));
  }
 }
}
#[test] fn debugger_python_records_judge_shapes() {
 if super::super::python::available("python3").is_err(){eprintln!("Skipping Python recorder: python3 missing");return;}
 let dir=Scratch::new().unwrap();
 for (meta,_,code,input,expected) in cases() {
  let meta:serde_json::Value=serde_json::from_str(meta).unwrap();let path=dir.0.join("solution.py");fs::write(&path,code).unwrap();
  let case=crate::runner::Case{id:0,input:input.into(),expected:None,custom:false};
  let trace=super::super::python::record("python3",&path,&meta,&case,Limits::default()).unwrap();assert_eq!(trace.error,None);
  if meta["name"]=="groupAnagrams" {let mut actual:Vec<Vec<String>>=serde_json::from_str(trace.output.as_deref().unwrap()).unwrap();actual.sort();let mut expected:Vec<Vec<String>>=serde_json::from_str(expected).unwrap();expected.sort();assert_eq!(actual,expected);}else{assert_eq!(trace.output.as_deref(),Some(expected));}
 }
}
#[test] fn debugger_cpp_limits_and_timeout() {
    if available().is_err(){eprintln!("Skipping C++ recorder: g++/Python-enabled gdb missing");return;}
    let dir=Scratch::new().unwrap();let path=dir.0.join("solution.cpp");
    let meta=serde_json::json!({"name":"solve","params":[{"type":"integer"}],"return":{"type":"integer"}});
    let case=crate::runner::Case{id:0,input:"3".into(),expected:None,custom:false};
    fs::write(&path,"class Solution {\npublic:\n int solve(int n) {\n  while(true) {\n   ++n;\n  }\n }\n};").unwrap();
    let trace=record(&path,&meta,&case,Limits{max_steps:7,..Limits::default()}).unwrap();assert!(trace.truncated);assert_eq!(trace.steps.len(),7);
    fs::write(&path,"class Solution {\npublic:\n int solve(int n) {\n  this_thread::sleep_for(chrono::seconds(5));\n  return n;\n }\n};").unwrap();
    let trace=record(&path,&meta,&case,Limits{timeout:Duration::from_millis(250),..Limits::default()}).unwrap();assert_eq!(trace.error.as_deref(),Some("Recording timed out"));
}
#[test] fn debugger_cpp_pretty_printer_containers_are_bounded() {
    if available().is_err(){eprintln!("Skipping C++ recorder: g++/Python-enabled gdb missing");return;}
    let dir=Scratch::new().unwrap();let path=dir.0.join("solution.cpp");
    fs::write(&path,"class Solution {\npublic:\n int solve(int n) {\n  deque<int> d(70,1);\n  stack<int> s(d);\n  queue<int> q(d);\n  priority_queue<int> h(d.begin(),d.end());\n  set<int> seen={1,2};\n  unordered_set<int> us={3,4};\n  pair<int,string> p={5,\"hello\"};\n  return n;\n }\n};").unwrap();
    let meta=serde_json::json!({"name":"solve","params":[{"type":"integer"}],"return":{"type":"integer"}});
    let case=crate::runner::Case{id:0,input:"3".into(),expected:None,custom:false};
    let trace=record(&path,&meta,&case,Limits::default()).unwrap();assert_eq!(trace.error,None);
    for (name,expected) in [("d","deque"),("s","stack"),("q","queue"),("h","heap")] {
        assert!(trace.steps.iter().flat_map(|s|&s.locals).any(|v|v.name==name && matches!(&v.value,super::super::Value::List{kind,items,truncated:6} if kind==expected && items.len()==64)),"{name} printer missing");
    }
    for name in ["seen","us"] {assert!(trace.steps.iter().flat_map(|s|&s.locals).any(|v|v.name==name && matches!(&v.value,super::super::Value::Set{items,..} if items.len()==2)));}
}
