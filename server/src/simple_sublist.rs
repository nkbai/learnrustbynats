use crate::error::{NError, Result, ERROR_SUBSCRIBTION_NOT_FOUND};
use bitflags::_core::cmp::Ordering;
use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::sync::Arc;

/**
为了讲解方便,考虑到Trie的实现以及Cache的实现都是很琐碎,
我这里专门实现一个简单的订阅关系查找,不支持*和>这两种模糊匹配.
这样就是简单的字符串查找了. 使用map即可.
但是为了后续的扩展性呢,我会定义SubListTrait,这样方便后续实现Trie树
*/
#[derive(Debug, Default)]
pub struct Subscription {
    pub subject: String,
    pub queue: Option<String>,
    pub sid: String,
}
#[derive(Debug, Default)]
pub struct SubResult {
    pub subs: Vec<ArcSubscription>,
    pub qsubs: Vec<Vec<ArcSubscription>>,
}
impl SubResult {
    pub fn is_empty(&self) -> bool {
        self.subs.len() == 0 && self.qsubs.len() == 0
    }
}
pub type ArcSubscription = Arc<Subscription>;
/*
因为孤儿原则,所以必须单独定义ArcSubscription
*/
#[derive(Debug, Clone)]
pub struct ArcSubscriptionWrapper(ArcSubscription);
impl std::cmp::PartialEq for ArcSubscriptionWrapper {
    fn eq(&self, other: &Self) -> bool {
        self.cmp(other) == Ordering::Equal
    }
}
/*
为了能够将ArcSubscription,必须实现下面这些Trait

*/
impl std::cmp::Eq for ArcSubscriptionWrapper {}
impl std::cmp::PartialOrd for ArcSubscriptionWrapper {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}
impl std::cmp::Ord for ArcSubscriptionWrapper {
    fn cmp(&self, other: &Self) -> Ordering {
        let a = self.0.as_ref() as *const Subscription as usize;
        let b = other.0.as_ref() as *const Subscription as usize;
        a.cmp(&b)
    }
}
pub type ArcSubResult = Arc<SubResult>;
pub trait SubListTrait {
    fn insert(&mut self, sub: Arc<Subscription>) -> Result<()>;
    fn remove(&mut self, sub: Arc<Subscription>) -> Result<()>;
    fn match_subject(&mut self, subject: &str) -> Result<ArcSubResult>;
}
#[derive(Debug, Default)]
struct SimpleSubList {
    subs: HashMap<String, BTreeSet<ArcSubscriptionWrapper>>,
    qsubs: HashMap<String, HashMap<String, BTreeSet<ArcSubscriptionWrapper>>>,
}

impl SubListTrait for SimpleSubList {
    fn insert(&mut self, sub: Arc<Subscription>) -> Result<()> {
        if let Some(ref q) = sub.queue {
            let entry = self
                .qsubs
                .entry(sub.subject.clone())
                .or_insert(Default::default());
            let queue = entry.entry(q.clone()).or_insert(Default::default());
            queue.insert(ArcSubscriptionWrapper(sub));
        } else {
            let subs = self
                .subs
                .entry(sub.subject.clone())
                .or_insert(Default::default());
            subs.insert(ArcSubscriptionWrapper(sub));
        }
        Ok(())
    }

    fn remove(&mut self, sub: Arc<Subscription>) -> Result<()> {
        if let Some(ref q) = sub.queue {
            if let Some(subs) = self.qsubs.get_mut(&sub.subject) {
                if let Some(qsubs) = subs.get_mut(q) {
                    qsubs.remove(&ArcSubscriptionWrapper(sub));
                } else {
                    return Err(NError::new(ERROR_SUBSCRIBTION_NOT_FOUND));
                }
            } else {
                return Err(NError::new(ERROR_SUBSCRIBTION_NOT_FOUND));
            }
        } else {
            if let Some(subs) = self.subs.get_mut(&sub.subject) {
                subs.remove(&ArcSubscriptionWrapper(sub));
            }
        }
        Ok(())
    }

    fn match_subject(&mut self, subject: &str) -> Result<ArcSubResult> {
        let mut r = SubResult::default();
        if let Some(subs) = self.subs.get(subject) {
            for s in subs {
                r.subs.push(s.0.clone());
            }
        }
        if let Some(qsubs) = self.qsubs.get(subject) {
            for (_, qsub) in qsubs {
                let mut v = Vec::with_capacity(qsub.len());
                for s in qsub {
                    v.push(s.0.clone());
                }
                r.qsubs.push(v);
            }
        }
        if r.is_empty() {
            Err(NError::new(ERROR_SUBSCRIBTION_NOT_FOUND))
        } else {
            Ok(Arc::new(r))
        }
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn test() {}
}
