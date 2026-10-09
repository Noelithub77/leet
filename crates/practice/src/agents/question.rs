//! A pending ACP question; the protocol worker waits without blocking the GUI.
use std::sync::{Arc, Mutex};
use serde_json::Value;
#[derive(Clone, Debug)]
pub struct Question { pub request: Value, response: Arc<Mutex<Option<Value>>> }
impl PartialEq for Question { fn eq(&self,other:&Self)->bool{Arc::ptr_eq(&self.response,&other.response)} }
impl Question {
    pub fn new(request:Value)->Self{Self{request,response:Arc::new(Mutex::new(None))}}
    pub fn answer(&self,response:Value){if let Ok(mut slot)=self.response.lock(){if slot.is_none(){*slot=Some(response);}}}
    pub(super) fn take(&self)->Option<Value>{self.response.lock().ok()?.take()}
}
