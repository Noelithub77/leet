#include <bits/stdc++.h>
using namespace std;
struct ListNode { int val; ListNode *next; ListNode():val(0),next(nullptr){} ListNode(int x):val(x),next(nullptr){} ListNode(int x,ListNode*n):val(x),next(n){} };
struct TreeNode { int val; TreeNode *left,*right; TreeNode():val(0),left(nullptr),right(nullptr){} TreeNode(int x):val(x),left(nullptr),right(nullptr){} TreeNode(int x,TreeNode*l,TreeNode*r):val(x),left(l),right(r){} };
namespace judge_driver {
struct Json { string scalar; vector<Json> items; bool null=false; };
struct Parser {
 string s; size_t p=0;
 void space(){while(p<s.size()&&isspace(static_cast<unsigned char>(s[p])))++p;}
 void utf8(string& out,unsigned x){if(x<128)out+=char(x);else if(x<2048){out+=char(192|(x>>6));out+=char(128|(x&63));}else if(x<65536){out+=char(224|(x>>12));out+=char(128|((x>>6)&63));out+=char(128|(x&63));}else{out+=char(240|(x>>18));out+=char(128|((x>>12)&63));out+=char(128|((x>>6)&63));out+=char(128|(x&63));}}
 unsigned hex(){if(p+4>s.size())throw runtime_error("Invalid Unicode escape");unsigned x=stoul(s.substr(p,4),nullptr,16);p+=4;return x;}
 Json parse(){
  space(); if(p>=s.size())throw runtime_error("Missing JSON argument"); Json j;
  if(s[p]=='['){++p;space();if(p<s.size()&&s[p]==']'){++p;return j;}while(true){j.items.push_back(parse());space();if(p>=s.size())throw runtime_error("Unclosed array");char c=s[p++];if(c==']')break;if(c!=',')throw runtime_error("Expected comma");}return j;}
  if(s[p]=='"'){++p;while(p<s.size()){char c=s[p++];if(c=='"')return j;if(c=='\\'){if(p>=s.size())throw runtime_error("Invalid escape");c=s[p++];switch(c){case 'n':c='\n';break;case 'r':c='\r';break;case 't':c='\t';break;case 'b':c='\b';break;case 'f':c='\f';break;case 'u':{unsigned x=hex();if(x>=0xd800&&x<=0xdbff){if(s.substr(p,2)!="\\u")throw runtime_error("Invalid surrogate");p+=2;unsigned y=hex();if(y<0xdc00||y>0xdfff)throw runtime_error("Invalid surrogate");x=0x10000+((x-0xd800)<<10)+(y-0xdc00);}utf8(j.scalar,x);continue;}}}j.scalar+=c;}throw runtime_error("Unclosed string");}
  size_t begin=p;while(p<s.size()&&s[p]!=','&&s[p]!=']'&&!isspace(static_cast<unsigned char>(s[p])))++p;j.scalar=s.substr(begin,p-begin);j.null=j.scalar=="null";return j;
 }
};
inline Json read(){string line;if(!getline(cin,line))throw runtime_error("Missing argument line");Parser p{line};Json j=p.parse();p.space();if(p.p!=line.size())throw runtime_error("Trailing JSON input");return j;}
template<class T> struct Decode {static T get(const Json&j){if constexpr(is_same_v<T,string>)return j.scalar;else if constexpr(is_same_v<T,char>){if(j.scalar.size()!=1)throw runtime_error("character requires one byte");return j.scalar[0];}else if constexpr(is_same_v<T,bool>)return j.scalar=="true";else if constexpr(is_integral_v<T>)return static_cast<T>(stoll(j.scalar));else return static_cast<T>(stod(j.scalar));}};
template<class T> struct Decode<vector<T>> {static vector<T> get(const Json&j){vector<T> out;for(auto &v:j.items)out.push_back(Decode<T>::get(v));return out;}};
template<> struct Decode<ListNode*> {static ListNode* get(const Json&j){ListNode dummy,*tail=&dummy;for(auto &v:j.items){tail->next=new ListNode(Decode<int>::get(v));tail=tail->next;}return dummy.next;}};
template<> struct Decode<TreeNode*> {static TreeNode* get(const Json&j){if(j.items.empty()||j.items[0].null)return nullptr;auto root=new TreeNode(Decode<int>::get(j.items[0]));queue<TreeNode*>q;q.push(root);size_t i=1;while(!q.empty()&&i<j.items.size()){auto n=q.front();q.pop();for(auto slot:{&n->left,&n->right}){if(i<j.items.size()&&!j.items[i].null){*slot=new TreeNode(Decode<int>::get(j.items[i]));q.push(*slot);}++i;}}return root;}};
inline string quote(const string&s){string out="\"";for(unsigned char c:s){switch(c){case '"':out+="\\\"";break;case '\\':out+="\\\\";break;case '\n':out+="\\n";break;case '\r':out+="\\r";break;case '\t':out+="\\t";break;default:if(c<32){char b[7];snprintf(b,7,"\\u%04x",c);out+=b;}else out+=char(c);}}return out+'"';}
inline string dump(const string&s){return quote(s);} inline string dump(char c){return quote(string(1,c));} inline string dump(bool b){return b?"true":"false";}
template<class T> string dump(T n){ostringstream out;out<<setprecision(17)<<n;return out.str();}
inline string dump(ListNode*n); inline string dump(TreeNode*n);
template<class T> string dump(const vector<T>&v){string out="[";bool first=true;for(const T&x:v){if(!first)out+=',';first=false;out+=dump(x);}return out+"]";}
inline string dump(ListNode*n){if(!n)return "null";vector<int>out;set<ListNode*>seen;while(n&&seen.insert(n).second&&out.size()<100000){out.push_back(n->val);n=n->next;}return dump(out);}
inline string dump(TreeNode*n){if(!n)return "null";vector<string>out;queue<TreeNode*>q;q.push(n);set<TreeNode*>seen;while(!q.empty()&&out.size()<100000){auto v=q.front();q.pop();if(!v){out.push_back("null");continue;}if(!seen.insert(v).second)throw runtime_error("Cyclic tree result");out.push_back(to_string(v->val));q.push(v->left);q.push(v->right);}while(!out.empty()&&out.back()=="null")out.pop_back();string result="[";for(size_t i=0;i<out.size();++i){if(i)result+=',';result+=out[i];}return result+"]";}
}
