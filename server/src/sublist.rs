/**
### 核心的trie树
这个算是整个系统稍微复杂一点的部分
核心就是一个Trie树

node是一个trie树
每个节点都是以.分割的字符串
foo.bar.aa
foo.cc.aa
foo.bb.dd
foo.dd
```
               foo
    /    /  |  \   \    \
    *    >  bar cc  bb  dd
    |			|   |   |
    aa	   aa	aa  aa
    ```
    当一个订阅foo.> 插入这个树上的时候, 这个订阅会放到>中去 ,称之为sub1
    当一个foo.* 插入的时候,订阅会放到* sub2
    当一个订阅foo.bar.aa 订阅来的时候会放到foo.bar.aa中去 sub3
    当有人再foo.ff 发布一个消息的时候会匹配到sub1,sub2
    当有人再foo.bar.aa发布一个消息的时候会匹配到sub2,sub3

    ### cache系统
    每次查找虽然是LogN,但是代价也挺大的,因此搞了缓存

    一个trie树遍历的缓存,当一个publisher发表一个消息的时候,很可能会针对这个主题再次发布消息,
    那么查找到的相关的所有的subscriber,可以缓存起来
    负面: 当新增或者删除subscriber的时候也要来cache里面遍历,修改.
*/
use crate::error::*;
use crate::simple_sublist::*;
use lru_cache::LruCache;
use std::collections::{BTreeSet, HashMap};
use std::sync::Arc;

const PWC: u8 = '*' as u8;
const FWC: u8 = '>' as u8;
const TSEP: &str = ".";
const BTSEP: u8 = '.' as u8;
// cacheMax is used to bound limit the frontend cache
const SL_CACHE_MAX: usize = 1024;
#[derive(Debug, Default)]
pub struct Level {
    pwc: Option<Box<TrieNode>>,            //*
    fwc: Option<Box<TrieNode>>,            //>
    nodes: HashMap<String, Box<TrieNode>>, //others
}
impl Level {
    pub fn is_empty(&self) -> bool {
        (self.pwc.is_none() || self.pwc.as_ref().unwrap().is_empty())
            && (self.fwc.is_none() || self.fwc.as_ref().unwrap().is_empty())
            && self.nodes.is_empty()
    }
}

#[derive(Debug, Default)]
pub struct TrieNode {
    next: Option<Box<Level>>,
    subs: BTreeSet<ArcSubscriptionWrapper>,
    qsubs: HashMap<String, BTreeSet<ArcSubscriptionWrapper>>,
}
impl TrieNode {
    fn new() -> Self {
        Self {
            next: None,
            subs: Default::default(),
            qsubs: Default::default(),
        }
    }
    fn is_empty(&self) -> bool {
        self.subs.is_empty()
            && self.qsubs.is_empty()
            && (self.next.is_none() || self.next.as_ref().unwrap().is_empty())
    }
}
#[derive(Debug)]
struct SubResultCache {
    cache: LruCache<String, ArcSubResult>,
}
impl SubResultCache {
    fn new(cache_size: usize) -> SubResultCache {
        Self {
            cache: LruCache::new(cache_size),
        }
    }
    /*
        插入的时候要考虑重建cache.
    比如插入一个a.>
    那么a.b.c a.d a.d.c 等对应的项中都要插入
    另外,cache是共享的,所以相应的项必须重建.
    */
    fn insert(&mut self, sub: ArcSubscription) {
        let mut v = Vec::new();
        for (subject, result) in self.cache.iter_mut() {
            if match_literal(subject, sub.subject.as_str()) {
                let mut r = SubResult::default();
                r.psubs = result.psubs.clone();
                r.qsubs = result.qsubs.clone();
                if let Some(ref q) = sub.queue {
                    let mut found = false;
                    for (pos, subs) in result.qsubs.iter().enumerate() {
                        if subs.get(0).unwrap().queue.as_ref().unwrap().as_str() == q.as_str() {
                            r.qsubs[pos].push(sub.clone());
                            found = true;
                        }
                    }
                    if !found {
                        r.qsubs.push(vec![sub.clone()]);
                    }
                } else {
                    r.psubs.push(sub.clone());
                }
                v.push((r, subject.clone()));
            }
        }
        for r in v {
            self.cache.insert(r.1, Arc::new(r.0));
        }
    }
    fn remove(&mut self, sub: &ArcSubscription) {
        let mut v = Vec::new();
        for (subject, result) in self.cache.iter_mut() {
            if match_literal(subject, sub.subject.as_str()) {
                let mut r = SubResult::default();
                r.psubs = result.psubs.clone();
                r.qsubs = result.qsubs.clone();
                if let Some(ref q) = sub.queue {
                    for (pos, subs) in result.qsubs.iter().enumerate() {
                        if subs.get(0).unwrap().queue.as_ref().unwrap().as_str() == q.as_str() {
                            for t in subs.iter().enumerate() {
                                if std::ptr::eq(t.1.as_ref(), sub.as_ref()) {
                                    r.qsubs[pos].swap_remove(t.0);
                                    break;
                                }
                            }
                            if r.qsubs[pos].len() == 0 {
                                r.qsubs.swap_remove(pos);
                            }
                            break;
                        }
                    }
                } else {
                    let pos = r
                        .psubs
                        .iter()
                        .position(|it| std::ptr::eq(it.as_ref(), sub.as_ref()));
                    if let Some(pos) = pos {
                        r.psubs.swap_remove(pos);
                    } else {
                        println!(" not found {:?}", sub);
                    }
                }
                v.push((r, subject.clone()));
            }
        }
        for r in v {
            self.cache.insert(r.1, Arc::new(r.0));
        }
    }
    fn get(&mut self, subject: &str) -> Option<ArcSubResult> {
        //        return Some(ArcSubResult::default());
        //todo 由于lru cache 自身问题,等修复后就不需要copy了
        self.cache.get_mut(subject).map(|r| Arc::clone(r))
        //        self.cache.get(&subject.to_string())
    }
    fn insert_result(&mut self, subject: &str, result: ArcSubResult) {
        self.cache.insert(subject.to_string(), result);
    }
}
#[test]
fn test_lru() {
    use lru::LruCache;
    let mut cache = LruCache::new(2);
    cache.put("apple", 3);
    cache.put("banana", 2);
    assert_eq!(*cache.get(&"apple").unwrap(), 3);
    assert_eq!(*cache.get(&"banana").unwrap(), 2);
    assert!(cache.get(&"pear").is_none());

    assert_eq!(cache.put("banana", 4), Some(2));
    assert_eq!(cache.put("pear", 5), None);

    assert_eq!(*cache.get(&"pear").unwrap(), 5);
    assert_eq!(*cache.get(&"banana").unwrap(), 4);
    assert!(cache.get(&"apple").is_none());

    {
        let v = cache.get_mut(&"banana").unwrap();
        *v = 6;
    }

    assert_eq!(*cache.get(&"banana").unwrap(), 6);
}
impl Default for SubResultCache {
    fn default() -> Self {
        Self::new(1024)
    }
}
#[derive(Debug, Default)]
pub struct TrieSubList {
    cache: SubResultCache,
    root: Level,
    d: ArcSubResult,
    default_node: Box<TrieNode>, //只是因为Insert的时候必须有一个初始化的值
}
impl TrieSubList {
    pub fn new() -> Self {
        Self {
            cache: Default::default(),
            root: Default::default(),
            d: ArcSubResult::default(),
            default_node: Default::default(),
        }
    }
}
impl SubListTrait for TrieSubList {
    /*
    将合法的subject插入树中,
    形如a.b.c a.*.c a.* a.>等
    插入的时候要考虑重建cache.
    比如插入一个a.>
    那么a.b.c a.d a.d.c 等对应的项中都要插入
    另外,cache是共享的,所以相应的项必须重建.
    */
    fn insert(&mut self, sub: Arc<Subscription>) -> Result<()> {
        if !is_valid_subject(sub.subject.as_str()) {
            return Err(NError::new(ERROR_INVALID_SUBJECT));
        }
        //        println!("insert {}", sub.subject);
        let mut l = &mut self.root;
        let mut n = &mut self.default_node;
        let mut tokens = split_subject(&sub.subject).peekable();
        while tokens.peek().is_some() {
            let token = tokens.next().unwrap();
            //            println!("token:{}", token);
            let t = token.as_bytes()[0];
            match t {
                PWC => {
                    if l.pwc.is_none() {
                        l.pwc = Some(Box::new(TrieNode::new()));
                    }
                    n = l.pwc.as_mut().unwrap();
                }
                FWC => {
                    if l.fwc.is_none() {
                        l.fwc = Some(Box::new(TrieNode::new()));
                    }
                    n = l.fwc.as_mut().unwrap();
                }
                _ => {
                    n = l
                        .nodes
                        .entry(token.to_string())
                        .or_insert(Box::new(TrieNode::new()));
                }
            }
            let is_last = tokens.peek().is_none();
            if is_last {
                break;
            }
            if n.next.is_none() {
                n.next = Some(Box::new(Level::default()));
            }
            l = n.next.as_mut().unwrap();
        }

        if let Some(ref q) = sub.queue {
            let qsubs = n.qsubs.entry(q.clone()).or_insert(Default::default());
            qsubs.insert(ArcSubscriptionWrapper(sub.clone()));
        } else {
            n.subs.insert(ArcSubscriptionWrapper(sub.clone()));
        }
        self.cache.insert(sub);
        Ok(())
    }
    /*
    将合法的subject从树中移除,
    形如a.b.c a.*.c a.* a.>等
    插入的时候要考虑重建cache.
    比如插入一个a.>
    那么a.b.c a.d a.d.c 等对应的项中都要删除
    另外,cache是共享的,所以相应的项必须重建.
    */
    fn remove(&mut self, sub: Arc<Subscription>) -> Result<()> {
        if !is_valid_subject(sub.subject.as_str()) {
            return Err(NError::new(ERROR_INVALID_SUBJECT));
        }
        let tokens = sub.subject.split(".").peekable();
        if Self::remove_internal(&mut self.root, tokens, &sub) {
            self.cache.remove(&sub);
        } else {
            return Err(NError::new(ERROR_SUBSCRIBTION_NOT_FOUND));
        }
        Ok(())
    }
    //pub 时用
    //pub a.b.c
    //需要查找到订阅了a.> a.*.c a.b.* a.b.c和> 这些可能匹配的节点
    //并且他们不应该在同一个queue中,就是订阅了a.*.c 和 a.b.c就算是他们有相同的queue,也不能做负载均衡.
    fn match_subject(&mut self, subject: &str) -> ArcSubResult {
        //        return Arc::new(SubResult::new());
        if let Some(r) = self.cache.get(subject) {
            return r;
        }
        if !is_valid_literal_subject(subject) {
            unreachable!("invalid subject {}", subject);
        }
        let mut r = Default::default();
        let tokens = split_subject(subject).peekable();
        let l = &mut self.root;
        Self::match_internal(l, tokens, &mut r);
        let r = Arc::new(r);
        self.cache.insert_result(subject, r.clone());
        r
    }
}
impl TrieSubList {
    fn cache_count(&self) -> usize {
        self.cache.cache.len()
    }
    fn add_node_to_result(n: &TrieNode, r: &mut SubResult) {
        for sub in n.subs.iter() {
            r.psubs.push(sub.0.clone());
        }
        for subs in n.qsubs.values() {
            let mut v = vec![];
            for sub in subs {
                v.push(sub.0.clone());
            }
            r.qsubs.push(v);
        }
    }
    fn match_internal(l: &mut Level, mut tokens: std::iter::Peekable<Split>, r: &mut SubResult) {
        let token = tokens.next();
        if token.is_none() {
            return;
        }
        let token = token.unwrap();
        let is_last = tokens.peek().is_none();
        //match >
        if let Some(ref fwc) = l.fwc {
            Self::add_node_to_result(fwc.as_ref(), r);
        }
        //match *
        if let Some(ref mut pwc) = l.pwc {
            if is_last {
                Self::add_node_to_result(pwc.as_ref(), r);
            } else if let Some(l) = pwc.next.as_mut() {
                Self::match_internal(l.as_mut(), tokens.clone(), r);
            }
        }
        //match exactly
        if let Some(n) = l.nodes.get_mut(token) {
            if is_last {
                Self::add_node_to_result(n.as_ref(), r);
            } else if let Some(ref mut l) = n.next {
                Self::match_internal(l.as_mut(), tokens, r);
            }
        }
    }
    //返回true表示删除了sub,否则表示没有删除
    fn remove_internal(
        l: &mut Level,
        mut tokens: std::iter::Peekable<std::str::Split<&str>>,
        sub: &Arc<Subscription>,
    ) -> bool {
        let token = tokens.next();
        if token.is_none() {
            return false;
        }
        let token = token.unwrap();
        let is_last = tokens.peek().is_none();
        let n;
        match token {
            "*" => {
                n = l.pwc.as_mut();
            }
            ">" => {
                n = l.fwc.as_mut();
            }
            _ => n = l.nodes.get_mut(token),
        }
        if n.is_none() {
            return false;
        }
        let n = n.unwrap();
        if is_last {
            if !Self::remove_sub(n.as_mut(), sub.clone()) {
                return false;
            }
        } else {
            if n.next.is_none() {
                return false;
            }
            let l = n.next.as_mut().unwrap();
            //如果成功移除了,就要考虑我这一层是否是空的了.
            if !Self::remove_internal(l.as_mut(), tokens, sub) {
                return false;
            }
        }
        if n.is_empty() {
            match token {
                "*" => {
                    l.pwc = None;
                }
                ">" => {
                    l.fwc = None;
                }
                _ => {
                    l.nodes.remove(token);
                }
            }
        }
        true
    }
    //从Node中移除一个sub
    fn remove_sub(n: &mut TrieNode, sub: ArcSubscription) -> bool {
        if let Some(ref q) = sub.queue {
            let qsubs = n.qsubs.get_mut(q);
            if let Some(qsubs) = qsubs {
                return qsubs.remove(&ArcSubscriptionWrapper(sub));
            }
        } else {
            return n.subs.remove(&ArcSubscriptionWrapper(sub));
        }
        false
    }
    fn match_test(&mut self, subject: &str) -> ArcSubResult {
        //        Arc::new(SubResult::new())
        self.d.clone()
    }
    fn match_test2(&mut self, subject: &str) -> Box<SubResult> {
        Box::new(SubResult::new())
    }
}
///is_valid_subject returns true if a subject is valid, false otherwise
/// 当收到sub消息时使用,无效的包括:
/// 1. 空的subject
/// 2. 连续的..
/// 3. 包含>,但是不以>结尾
pub fn is_valid_subject(subject: &str) -> bool {
    if subject.len() == 0 {
        return false;
    }
    let mut sfwc = false;
    !split_subject(subject).any(|s: &str| {
        //连续的..或者>后面有.
        if s.len() == 0 || sfwc {
            return true;
        }
        if s.len() >= 1 {
            if s.as_bytes()[0] == FWC {
                //只允许单独的>
                if s.len() > 1 {
                    return true;
                }
                sfwc = true;
            }
        }
        false
    })
}

/// pub时用的
/// 无效的主题 包括:
/// 1. is_valid_subject 认为无效的肯定无效
/// 2. 包含*
/// 3. 包含>
pub fn is_valid_literal_subject(subject: &str) -> bool {
    if subject.len() == 0 {
        return false;
    }
    //    split_subject(subject).any()
    !split_subject(subject).any(|s: &str| {
        if s.len() == 0 {
            return true;
        }
        if s.len() > 1 {
            return false;
        }
        let b = s.as_bytes()[0];
        if b == FWC || b == PWC {
            return true;
        }
        false
    })
}
#[derive(Clone, Debug)]
struct Split<'a> {
    pos: usize,
    buf: &'a [u8],
}
fn split_subject<'a>(subject: &'a str) -> Split<'a> {
    Split {
        pos: 0,
        buf: subject.as_bytes(),
    }
}
impl<'a> std::iter::Iterator for Split<'a> {
    type Item = &'a str;
    fn next(&mut self) -> Option<Self::Item> {
        if self.pos > self.buf.len() {
            return None;
        }
        let start = self.pos;
        while self.pos < self.buf.len() {
            if self.buf[self.pos] == BTSEP {
                break;
            }
            self.pos += 1;
        }
        let str = unsafe { std::str::from_utf8_unchecked(&self.buf[start..self.pos]) };
        self.pos += 1; //无论哪种情况,都应该跳过.
        return Some(str);
    }
}
#[test]
fn test_split() {
    let v = vec!["a", "b", "c"];
    let r: Vec<_> = split_subject("a.b.c").collect();
    assert_eq!(v, r);
    let v = vec!["a", "b", "", "c"];
    let r: Vec<_> = split_subject("a.b..c").collect();
    assert_eq!(v, r);
    let v = vec!["", "", "a", "b", "c"];
    let r: Vec<_> = split_subject("..a.b.c").collect();
    assert_eq!(v, r);
    let v = vec!["a", "b", "c", ""];
    let r: Vec<_> = split_subject("a.b.c.").collect();
    assert_eq!(v, r);
}
/// matchLiteral is used to test literal subjects, those that do not have any
/// wildcards, with a target subject. This is used in the cache layer.
/// 判断a.b.c和a.*.c时否匹配
fn match_literal(literal: &str, subject: &str) -> bool {
    let mut literal_iter = split_subject(literal).peekable();
    let mut subject_iter = split_subject(subject).peekable();

    while literal_iter.peek().is_some() {
        //a.b.c走完了,即使是a.b.c.>也不能匹配
        if subject_iter.peek().is_none() {
            return false;
        }
        let literal = literal_iter.next().unwrap();
        let subject = subject_iter.next().unwrap();
        if literal == subject {
            continue;
        }
        if subject == "*" {
            continue;
        }
        if subject == ">" {
            return true;
        }
        return false;
    }
    subject_iter.peek().is_none()
}
#[cfg(test)]
fn test_new_sub(subject: &str) -> Subscription {
    use crate::client::new_test_tcp_writer;
    let writer = new_test_tcp_writer();
    Subscription::new(subject, None, "1", writer.clone())
}
#[cfg(test)]
fn test_new_sub_arc(subject: &str) -> ArcSubscription {
    Arc::new(test_new_sub(subject))
}
#[cfg(test)]
mod tests {
    use super::*;
    fn verify_count(sub: &TrieSubList, count: usize) {
        //        assert_eq!(
        //            sub.count(),
        //            count,
        //            "expect count={},got={}",
        //            count,
        //            sub.count()
        //        );
    }
    fn verify_len<T>(r: &[T], l: usize) {
        assert_eq!(r.len(), l, "results len expect={},got={}", l, r.len());
    }
    fn verify_qlen(r: &Vec<Vec<Arc<Subscription>>>, l: usize) {
        assert_eq!(r.len(), l, "queue results len expect={},got={}", l, r.len());
    }
    fn verify_num_levels(s: &TrieSubList, l: usize) {
        //        assert_eq!(
        //            s.num_levels(),
        //            l,
        //            "numlevels expect={},got={}",
        //            l,
        //            s.num_levels()
        //        );
    }

    fn verify_member(r: &[Arc<Subscription>], val: &Subscription) {
        for s in r {
            if std::ptr::eq(s.as_ref(), val) {
                return;
            }
        }
        assert!(false, "sub:{:?} not found in results", val);
    }
    fn new_qsub(subject: &str, queue: &str) -> Subscription {
        let mut s = test_new_sub(subject);
        s.queue = Some(queue.into());
        s
    }
    #[test]
    fn test_match_literal() {
        assert_eq!(true, match_literal("a.b.c", "a.*.c"));
        assert_eq!(true, match_literal("a.b.c", "a.>"));
        assert_eq!(false, match_literal("a.b.c.d", "a.*.c"));
    }

    #[test]
    fn test_sublist_insert_count() {
        let mut s = TrieSubList::new();
        assert!(s.insert(Arc::new(test_new_sub("foo"))).is_ok());
        assert!(s.insert(Arc::new(test_new_sub("bar"))).is_ok());
        assert!(s.insert(Arc::new(test_new_sub("foo.bar"))).is_ok());
        verify_count(&s, 3);
    }
    #[test]
    fn test_sublist_simple() {
        let mut s = TrieSubList::new();
        let subject = "foo";
        let sub = Arc::new(test_new_sub(subject));
        assert!(s.insert(sub.clone()).is_ok());
        let r = s.match_subject(subject);
        verify_len(r.psubs.as_slice(), 1);
        verify_member(r.psubs.as_slice(), sub.as_ref());
    }
    #[test]
    fn test_sublist_simple_multi_tokens() {
        let mut s = TrieSubList::new();
        let subject = "foo.bar.baz";
        let sub = Arc::new(test_new_sub(subject));
        assert!(s.insert(sub.clone()).is_ok());
        let r = s.match_subject(subject);
        verify_len(r.psubs.as_slice(), 1);
        verify_member(r.psubs.as_slice(), sub.as_ref());
    }
    #[test]
    fn test_sublist_partial_wildcard() {
        let mut s = TrieSubList::new();
        let lsub = Arc::new(test_new_sub("a.b.c"));
        let psub = Arc::new(test_new_sub("a.*.c"));

        assert!(s.insert(lsub.clone()).is_ok());
        assert!(s.insert(psub.clone()).is_ok());
        let r = s.match_subject(&lsub.subject);
        verify_len(r.psubs.as_slice(), 2);
        verify_member(r.psubs.as_slice(), lsub.as_ref());
        verify_member(r.psubs.as_slice(), psub.as_ref());
    }
    #[test]
    fn test_sublist_partial_wildcard_at_end() {
        let mut s = TrieSubList::new();
        let lsub = Arc::new(test_new_sub("a.b.c"));
        let psub = Arc::new(test_new_sub("a.b.*"));

        assert!(s.insert(lsub.clone()).is_ok());
        assert!(s.insert(psub.clone()).is_ok());
        let r = s.match_subject(&lsub.subject);
        verify_len(r.psubs.as_slice(), 2);
        verify_member(r.psubs.as_slice(), lsub.as_ref());
        verify_member(r.psubs.as_slice(), psub.as_ref());
    }
    #[test]
    fn test_sublist_partial_full_wildcard() {
        let mut s = TrieSubList::new();
        let lsub = Arc::new(test_new_sub("a.b.c"));
        let psub = Arc::new(test_new_sub("a.>"));

        assert!(s.insert(lsub.clone()).is_ok());
        assert!(s.insert(psub.clone()).is_ok());
        let r = s.match_subject(&lsub.subject);
        verify_len(r.psubs.as_slice(), 2);
        verify_member(r.psubs.as_slice(), lsub.as_ref());
        verify_member(r.psubs.as_slice(), psub.as_ref());
    }
    #[test]
    fn test_sublist_remove() {
        let mut s = TrieSubList::new();
        let sub = Arc::new(test_new_sub("a.b.c.d"));

        assert!(s.insert(sub.clone()).is_ok());
        let r = s.match_subject(&sub.subject);
        verify_len(r.psubs.as_slice(), 1);
        verify_count(&s, 1);
        assert!(s.remove(Arc::new(test_new_sub("a.b.c"))).is_err());
        verify_count(&s, 1);
        assert!(s.remove(sub.clone()).is_ok());
        verify_count(&s, 0);
        let r = s.match_subject(&sub.subject);
        verify_len(r.psubs.as_slice(), 0);
    }

    #[test]
    fn test_sublist_remove_wildcard() {
        let mut s = TrieSubList::new();
        let sub = Arc::new(test_new_sub("a.b.c.d"));
        let psub = Arc::new(test_new_sub("a.b.*.d"));
        let fsub = Arc::new(test_new_sub("a.b.>"));
        let _ = s.insert(sub.clone());
        let _ = s.insert(psub.clone());
        let _ = s.insert(fsub.clone());
        verify_count(&s, 3);

        let r = s.match_subject(&sub.subject);
        verify_len(r.psubs.as_slice(), 3);
        assert!(s.remove(sub.clone()).is_ok());
        verify_count(&s, 2);
        assert!(s.remove(fsub.clone()).is_ok());
        verify_count(&s, 1);
        assert!(s.remove(psub.clone()).is_ok());
        verify_count(&s, 0);

        verify_count(&s, 1);
        assert!(s.remove(Arc::new(test_new_sub("a.b.c"))).is_err());
        verify_count(&s, 0);

        let r = s.match_subject(&sub.subject);
        verify_len(r.psubs.as_slice(), 0);
    }
    #[test]
    fn test_sublist_remove_cleanup() {
        let mut s = TrieSubList::new();
        let literal = "a.b.c.d.e.f";
        let depth = literal.split(TSEP).count();
        let sub = Arc::new(test_new_sub(literal));
        verify_num_levels(&s, 0);
        let _ = s.insert(sub.clone());
        verify_num_levels(&s, depth);
        let _ = s.remove(sub.clone());
        verify_num_levels(&s, 0);
    }
    #[test]
    fn test_sublist_remove_cleanup_wildcards() {
        let mut s = TrieSubList::new();
        let literal = "a.b.*.d.e.>";
        let depth = literal.split(TSEP).count();
        let sub = Arc::new(test_new_sub(literal));
        verify_num_levels(&s, 0);
        let _ = s.insert(sub.clone());
        verify_num_levels(&s, depth);
        let _ = s.remove(sub);
        verify_num_levels(&s, 0);
    }
    #[test]
    fn test_sublist_invalid_subjects_insert() {
        let mut s = TrieSubList::new();
        assert!(s.insert(Arc::new(test_new_sub(".foo"))).is_err());
        assert!(s.insert(Arc::new(test_new_sub("foo."))).is_err());
        assert!(s.insert(Arc::new(test_new_sub("foo..bar"))).is_err());
        assert!(s.insert(Arc::new(test_new_sub("foo.bar..baz"))).is_err());
        assert!(s.insert(Arc::new(test_new_sub("foo.>.bar"))).is_err());
    }
    #[test]
    fn test_sublist_cache() {
        let mut s = TrieSubList::new();
        let subject = "a.b.c.d";
        let sub = Arc::new(test_new_sub(subject));
        let psub = Arc::new(test_new_sub("a.b.*.d"));
        let fsub = Arc::new(test_new_sub("a.b.>"));
        let _ = s.insert(sub.clone());
        let _ = s.insert(psub.clone());
        let _ = s.insert(fsub.clone());
        verify_count(&s, 3);
        let r = s.match_subject(subject);
        verify_len(r.psubs.as_slice(), 3);
        let _ = s.remove(sub);
        verify_count(&s, 2);
        let _ = s.remove(fsub.clone());
        verify_count(&s, 1);
        let _ = s.remove(psub.clone());
        verify_count(&s, 0);
        assert_eq!(s.cache_count(), 0);
        let r = s.match_subject(subject);
        verify_len(r.psubs.as_slice(), 0);
        for i in 0..2 * SL_CACHE_MAX {
            s.match_subject(format!("foo-#{}", i).as_str());
        }
        //        assert!(s.cache_count() <= SL_CACHE_MAX);
    }
    #[test]
    fn test_sublist_basic_queue_results() {
        let mut s = TrieSubList::new();
        let subject = "foo";
        let sub = Arc::new(test_new_sub(subject));
        let sub1 = Arc::new(new_qsub(subject, "bar"));
        let sub2 = Arc::new(new_qsub(subject, "baz"));

        let _ = s.insert(sub1.clone());
        let r = s.match_subject(subject);
        verify_len(r.psubs.as_slice(), 0);
        verify_qlen(&r.qsubs, 1);
        verify_len(r.qsubs[0].as_slice(), 1);
        verify_member(r.qsubs[0].as_slice(), sub1.as_ref());

        let _ = s.insert(sub2.clone());
        let r = s.match_subject(subject);
        verify_len(r.psubs.as_slice(), 0);
        println!("qsubs={:?}", r.qsubs);
        verify_qlen(&r.qsubs, 2);
        verify_len(r.qsubs[0].as_slice(), 1);
        verify_len(r.qsubs[1].as_slice(), 1);
        verify_member(r.qsubs[0].as_slice(), sub1.as_ref());
        verify_member(r.qsubs[1].as_slice(), sub2.as_ref());
    }
    #[test]
    fn test_valid_literal_subject() {
        assert_eq!(is_valid_literal_subject("foo"), true);
        assert_eq!(is_valid_literal_subject(".foo"), false);
        assert_eq!(is_valid_literal_subject("foo."), false);
        assert_eq!(is_valid_literal_subject("foo..bar"), false);
        assert_eq!(is_valid_literal_subject("foo.bar.*"), false);
        assert_eq!(is_valid_literal_subject("foo.bar.>"), false);
        assert_eq!(is_valid_literal_subject("*"), false);
        assert_eq!(is_valid_literal_subject(">"), false);
    }
    #[test]
    fn test_match_literal2() {
        println!("a={}", 3);
        assert_eq!(match_literal("foo", "foo"), true);
        assert_eq!(match_literal("foo", "bar"), false);
        assert_eq!(match_literal("foo", "*"), true);
        assert_eq!(match_literal("foo", ">"), true);
        assert_eq!(match_literal("foo.bar", ">"), true);
        assert_eq!(match_literal("foo.bar", "foo.>"), true);
        assert_eq!(match_literal("foo.bar", "bar.>"), false);
        assert_eq!(match_literal("stats.test.22", "stats.>"), true);
        assert_eq!(match_literal("stats.test.22", "stats.*.*"), true);
        assert_eq!(match_literal("foo.bar", "foo"), false);
        assert_eq!(match_literal("stats.test.foos", "stats.test.foos"), true);
        assert_eq!(match_literal("stats.test.foos", "stats.test.foo"), false);
    }
    #[test]
    fn test_sublist_two_token_pub_match_single_token_sub() {
        let mut s = TrieSubList::new();
        let sub = Arc::new(test_new_sub("foo"));
        let _ = s.insert(sub.clone());
        let r = s.match_subject("foo");
        verify_len(r.psubs.as_slice(), 1);
        let r = s.match_subject("foo.bar");
        verify_len(r.psubs.as_slice(), 0);
    }
}

#[cfg(test)]
mod benchmark {
    extern crate test;
    use super::*;
    use test::Bencher;

    fn create_subs(pre: &str, subs: &mut Vec<Arc<Subscription>>) {
        let tokens = vec![
            "apcera",
            "continuum",
            "component",
            "router",
            "api",
            "imgr",
            "jmgr",
            "auth",
        ];
        for t in tokens {
            let sub;
            if pre.len() > 0 {
                sub = format!("{}.{}", pre, t);
            } else {
                sub = t.into();
            }
            subs.push(Arc::new(test_new_sub(sub.as_str())));
            if sub.split(TSEP).count() < 5 {
                create_subs(sub.as_str(), subs);
            }
        }
    }
    fn add_wild_cards(s: &mut TrieSubList) {
        let _ = s.insert(Arc::new(test_new_sub("cloud.>")));
        let _ = s.insert(Arc::new(test_new_sub("cloud.continuum.component.>")));
        let _ = s.insert(Arc::new(test_new_sub("cloud.*.*.router.*")));
    }
    fn create_test_subs() -> Vec<Arc<Subscription>> {
        let mut subs = Vec::new();
        create_subs("", &mut subs);
        subs
    }
    fn get_test_sublist() -> TrieSubList {
        let mut s = TrieSubList::new();
        let subs = create_test_subs();
        for sub in subs {
            let _ = s.insert(sub.clone());
        }
        add_wild_cards(&mut s);
        s
    }
    #[test]
    fn test_subs() {
        let subs = create_test_subs();
        let l = subs.len();
        println!("subs len={}", l);
        subs.iter().take(1000).for_each(|sub| {
            println!("subs={:?}", sub.subject);
        });
    }
    use lazy_static::lazy_static;
    lazy_static! {
        static ref TEST_SUBS_COLLECT: Vec<ArcSubscription> = { create_test_subs() };
    }
    #[bench]
    fn benchmark1_sublist_insert(b: &mut Bencher) {
        //为什么go版本的insert只需要300ns,我这个需要3000ns,慢了一个数量级
        let mut s = TrieSubList::new();
        let subs = &TEST_SUBS_COLLECT;
        let l = subs.len();
        let mut i = 0;
        //        println!("subs len={}", l);
        //        println!("subs={:?}", subs);
        b.iter(|| {
            let _ = s.insert(subs[i % l].clone());
            //                        println!("i={}", i);
            i += 1;
        });
        println!("count={}", i);
    }

    #[bench]
    fn benchmark1_match_single_token(b: &mut Bencher) {
        let mut s = get_test_sublist();
        b.iter(|| {
            let _ = s.match_subject("apcera");
        })
    }
    #[bench]
    fn benchmark1_match_twotokens(b: &mut Bencher) {
        let mut s = get_test_sublist();
        b.iter(|| {
            let _ = s.match_subject("apcera.continuum");
        })
    }
    //    #[test]
    //    fn test_match() {
    //        let mut s = get_test_sublist();
    //        for i in 0..10 {
    //            let _ = s.match_subject("apcera.continuum.component");
    //        }
    //    }
    #[bench]
    fn benchmark1_match_threetokens(b: &mut Bencher) {
        let mut s = get_test_sublist();
        b.iter(|| {
            let _ = s.match_subject("apcera.continuum.component");
        })
    }
    #[bench]
    fn benchmark1_match_fourtokens(b: &mut Bencher) {
        let mut s = get_test_sublist();
        let _ = s.match_subject("apcera.continuum.component.router");
        let summary = b.bench(|b| {
            b.iter(|| {
                let _ = s.match_subject("apcera.continuum.component.router");
            });
        });
        println!("summary={:?}", summary)
    }
    #[bench]
    fn benchmark1_match_fivetokens2(b: &mut Bencher) {
        let mut s = get_test_sublist();
        b.iter(|| {
            let _ = s.match_test("apcera.continuum.component.router.ZZZZ");
        })
    }
    #[bench]
    fn benchmark1_match_fivetokens3(b: &mut Bencher) {
        let mut s = get_test_sublist();
        b.iter(|| {
            let _ = s.match_test2("apcera.continuum.component.router.ZZZZ");
        })
    }
    #[bench]
    fn benchmark1_match_fivetokens(b: &mut Bencher) {
        let mut s = get_test_sublist();
        b.iter(|| {
            let _ = s.match_subject("apcera.continuum.component.router.ZZZZ");
        })
    }
    fn get_test_array() -> Vec<u32> {
        let mut v = vec![32; 10000];
        v[9000] = 999;
        return v;
    }
    fn search_order(v: &[u32]) -> i32 {
        for i in 0..v.len() {
            if v[i] == 999 {
                return i as i32;
            }
        }
        return -1;
    }
    //因为不能越界,确保最后一个不是999
    fn search_order2(v: &mut [u32]) -> i32 {
        let l = v.len() - 1;
        let hold = v[l];
        v[l] = 999;
        let mut i = 0;
        loop {
            if v[i] == 999 {
                break;
            }
            i += 1;
        }
        v[l] = hold;
        if i == l {
            return -1;
        } else {
            return i as i32;
        }
    }
    fn search_order3(v: &[u32]) -> i32 {
        let l = v.len() - 1;
        //        let hold = v[l];
        //        v[l] = 999;
        let mut i = 0;
        //访问越界
        while i <= l {
            if v[i] == 999 {
                break;
            }
            if v[i + 1] == 999 {
                i += 1;
                break;
            }
            if v[i + 2] == 999 {
                i += 2;
                break;
            }
            if v[i + 3] == 999 {
                i += 3;
                break;
            }
            if v[i + 4] == 999 {
                i += 4;
                break;
            }
            if v[i + 5] == 999 {
                i += 5;
                break;
            }
            if v[i + 6] == 999 {
                i += 6;
                break;
            }
            if v[i + 7] == 999 {
                i += 7;
                break;
            }
            if v[i + 8] == 999 {
                i += 8;
                break;
            }
            i += 8;
        }
        //        v[l] = holxd;
        if i > l {
            return -1;
        } else {
            return i as i32;
        }
    }
    #[bench]
    fn test_search_order(b: &mut Bencher) {
        let v = get_test_array();
        b.iter(|| {
            assert_eq!(9000, search_order(v.as_slice()));
        })
    }
    #[bench]
    fn test_search_order2(b: &mut Bencher) {
        let mut v = get_test_array();
        b.iter(|| {
            assert_eq!(9000, search_order2(v.as_mut_slice()));
        })
    }
    #[bench]
    fn test_search_order3(b: &mut Bencher) {
        let mut v = get_test_array();
        b.iter(|| {
            assert_eq!(9000, search_order3(v.as_slice()));
        })
    }
}
