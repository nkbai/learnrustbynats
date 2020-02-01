use std::collections::HashMap;
use std::sync::Arc;

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
const PWC: &str = "*";
const FWC: &str = ">";
type ArcSubscription = Arc<Subscription>;
type ArcSubResult = Arc<SubResult>;
#[derive(Debug, Default)]
pub struct Level {
    pwc: Option<Box<TrieNode>>,            //*
    fwc: Option<Box<TrieNode>>,            //>
    nodes: HashMap<String, Box<TrieNode>>, //>
}
#[derive(Debug, Default)]
pub struct TrieNode {
    next: Option<Box<Level>>,
    subs: Vec<Arc<Subscription>>,
    qsubs: Vec<Vec<ArcSubscription>>,
}
impl TrieNode {
    fn new() -> Self {
        Self {
            next: None,
            subs: vec![],
            qsubs: vec![],
        }
    }
}
#[derive(Debug, Default)]
pub struct Subscription {
    subject: String,
    queue: Option<String>,
    sid: String,
}
#[derive(Debug, Default)]
pub struct SubResult {
    subs: Vec<ArcSubscription>,
    qsubs: Vec<Vec<ArcSubscription>>,
}
#[derive(Debug, Default)]
struct SubList {
    cache: HashMap<String, ArcSubResult>,
    root: Level,
}
impl SubList {
    pub fn new() -> Self {
        Self {
            cache: Default::default(),
            root: Default::default(),
        }
    }
    pub fn insert(&mut self, sub: Arc<Subscription>) -> Result<()> {
        let mut l = &mut self.root;
        let mut n = &mut Box::new(TrieNode::new());
        for token in sub.subject.split(".") {
            println!("token:{}", token);
            match token {
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
            if n.next.is_none() {
                n.next = Some(Box::new(Level::default()));
            }
            l = n.next.as_mut().unwrap();
        }
        if sub.queue.is_none() {
            n.subs.push(sub);
            return Ok(());
        }
        let qstr = sub.queue.as_ref().unwrap().as_str();
        //如果有queue就会麻烦,
        let pos = n.qsubs.iter_mut().find(|q| {
            let q = q.get(0).unwrap().queue.as_ref().unwrap();
            q == qstr
        });
        if let Some(q) = pos {
            q.push(sub);
        } else {
            n.qsubs.push(vec![sub]);
        }
        Ok(())
    }
    pub fn remove(&mut self, sub: Arc<Subscription>) -> Result<()> {
        Err(NError::new(ERROR_INVALID_SUBJECT))
    }
    pub fn match_subject(&mut self, subject: &str) -> Result<ArcSubResult> {
        Err(NError::new(ERROR_INVALID_SUBJECT))
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn test() {}
    #[test]
    fn test_insert() {
        let mut sl = SubList::new();
        let mut sub = Arc::new(Subscription {
            subject: "a.b.c".to_string(),
            queue: None,
            sid: "1".to_string(),
        });
        let r = sl.insert(sub.clone());
        assert!(r.is_ok());
        let nodea = sl.root.nodes.get("a");
        if nodea.is_none() {
            assert!(false, "node a must exist");
            return;
        }
        let nodea = nodea.unwrap();
        let nodeb = nodea.next.as_ref().unwrap().nodes.get("b");
        if nodeb.is_none() {
            assert!(false, "node b must exist");
            return;
        }
        let nodeb = nodeb.unwrap();
        let nodec = nodeb.next.as_ref().unwrap().nodes.get("c");
        if nodec.is_none() {
            assert!(false, "node c must exist");
            return;
        }
        let nodec = nodec.unwrap();
        println!("nodec.next is none:{}", nodec.as_ref().next.is_none());
        assert_eq!(nodec.subs.len(), 1);
    }
}
