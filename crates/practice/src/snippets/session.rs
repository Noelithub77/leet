//! Tracks byte ranges while the user edits a placeholder and its mirrors.
use std::ops::Range;
use super::body::{Expansion, Stop};

pub struct Session { pub stops: Vec<Stop>, pub active: usize, previous: String }
impl Session {
    pub fn new(expansion: Expansion, offset: usize, document: String) -> Self {
        let mut stops = expansion.stops;
        for stop in &mut stops { for range in &mut stop.ranges { range.start += offset; range.end += offset; } }
        Self { stops, active: 0, previous: document }
    }
    pub fn range(&self) -> Range<usize> { self.stops[self.active].range() }
    pub fn next(&mut self, backwards: bool) -> Option<Range<usize>> {
        if backwards { self.active = self.active.saturating_sub(1); }
        else if self.active + 1 < self.stops.len() { self.active += 1; }
        else { return None; }
        Some(self.range())
    }
    /// Returns mirror replacements in descending order. Edits outside the active stop cancel.
    pub fn changed(&mut self, document: &str) -> Option<Vec<(Range<usize>, String)>> {
        if document == self.previous { return Some(Vec::new()); }
        let before = &self.previous;
        let mut start = before.bytes().zip(document.bytes()).take_while(|(a,b)| a == b).count();
        while !before.is_char_boundary(start) || !document.is_char_boundary(start) { start -= 1; }
        let mut suffix = before[start..].bytes().rev().zip(document[start..].bytes().rev()).take_while(|(a,b)| a == b).count();
        while !before.is_char_boundary(before.len() - suffix) || !document.is_char_boundary(document.len() - suffix) { suffix -= 1; }
        let old = start..before.len() - suffix;
        let new_end = document.len() - suffix;
        let active = self.range();
        if old.start < active.start || old.end > active.end { return None; }
        let index = self.stops[self.active].index;
        let primary = self.stops[self.active].primary;
        self.adjust(&old, new_end - start, (index, primary));
        let active = self.range();
        let mut ancestors:Vec<_>=self.stops.iter().filter(|s|s.index!=index&&s.index!=0&&s.range().start<=active.start&&s.range().end>=active.end)
            .map(|s|(s.range().end-s.range().start,s.index)).collect();
        ancestors.sort_unstable();
        let mut result=document.to_owned();let mut all_edits=Vec::new();
        for index in std::iter::once(index).chain(ancestors.into_iter().map(|(_,index)|index)) {
            let stop=self.stops.iter().find(|s|s.index==index)?;
            let value=result.get(stop.range())?.to_owned();
            let mut edits:Vec<_>=stop.ranges.iter().enumerate().filter(|(ix,_)|*ix!=stop.primary)
                .map(|(ix,range)|(range.clone(),stop.transforms[ix].as_ref().map_or_else(||value.clone(),|t|t.apply(&value)),ix)).collect();
            edits.sort_by_key(|(range,_,ix)|std::cmp::Reverse((range.start,*ix)));
            for(range,text,ix)in edits {
                result.get(range.clone())?;result.replace_range(range.clone(),&text);
                self.adjust(&range,text.len(),(index,ix));all_edits.push((range,text));
            }
        }
        self.previous=result;Some(all_edits)
    }

    fn adjust(&mut self, edited: &Range<usize>, length: usize, target: (u32,usize)) {
        let delta = length as isize - (edited.end - edited.start) as isize;
        let active_index = self.stops[self.active].index;
        // Replacing an outer placeholder invalidates nested stops.
        self.stops.retain(|stop| { let range = stop.range(); stop.index == active_index || edited.is_empty() || !(range.start >= edited.start && range.end <= edited.end && range.start < edited.end) });
        for stop in &mut self.stops {
            for (ix, range) in stop.ranges.iter_mut().enumerate() {
                if (stop.index,ix) == target {
                    range.end = range.end.saturating_add_signed(delta);
                } else if range.start >= edited.end && (range.start > edited.start || !edited.is_empty() || range.start == range.end && (stop.index == 0 || stop.index == target.0 && ix > target.1)) {
                    range.start = range.start.saturating_add_signed(delta); range.end = range.end.saturating_add_signed(delta);
                } else if range.start <= edited.start && range.end >= edited.end && range.end > edited.start {
                    range.end = range.end.saturating_add_signed(delta);
                }
            }
        }
        // Drop invalidated nested stops and keep the current stop selected.
        self.stops.retain(|stop| !stop.ranges.is_empty());
        self.active = self.stops.iter().position(|s| s.index == active_index).unwrap_or(0);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::snippets::body;
    #[test]
    fn mirrors_shift_unicode_and_final_cursor() {
        let e = body::preview("${1:i} $1 ${1/(.*)/${1:/upcase}/} $0");
        let mut s = Session::new(e.clone(), 0, e.text);
        let edits = s.changed("é i I ").unwrap();
        assert_eq!(edits.len(), 2);
        assert_eq!(s.previous, "é é É ");
        assert_eq!(s.next(false), Some(9..9));
        assert!(s.changed("outside").is_none());
    }
    #[test]
    fn adjacent_empty_mirrors_and_final_cursor() {
        let e = body::preview("$1$1$0");
        let mut s=Session::new(e.clone(),0,e.text);
        s.changed("x").unwrap();
        assert_eq!(s.previous,"xx");
        assert_eq!(s.range(),0..1);
        assert_eq!(s.next(false),Some(2..2));
    }
    #[test]
    fn editing_nested_stop_updates_parent_mirrors() {
        let e=body::preview("${1:vector<${2:int}>} $1 $0");
        let mut s=Session::new(e.clone(),0,e.text);
        s.next(false);s.changed("vector<long> vector<int> ").unwrap();
        assert_eq!(s.previous,"vector<long> vector<long> ");
    }
    #[test]
    fn replacing_parent_removes_nested_stops() {
        let e = body::preview("${1:vector<${2:int}>} $0");
        let mut s = Session::new(e.clone(), 0, e.text);
        s.changed("set ").unwrap();
        assert_eq!(s.stops.len(), 2);
        assert_eq!(s.next(false), Some(4..4));
    }
}
