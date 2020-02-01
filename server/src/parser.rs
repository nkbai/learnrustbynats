/**
## pub
```
PUB <subject> <size>\r\n
<message>\r\n
```
## sub
```
SUB <subject> <sid>\r\n
SUB <subject> <queue> <sid>\r\n
```
## MSG
```
MSG <subject> <sid> <size>\r\n
<message>\r\n
```
*/
use crate::error::*;
#[macro_export]
macro_rules! parse_error {
    ( ) => {{
        return Err(NError::new(ERROR_PARSE));
    }};
}

#[derive(Debug, Clone)]
enum ParseState {
    OpStart,
    OpS,
    OpSu,
    OpSub,
    OPSubSpace,
    OpSubArg,
    OpP,
    OpPu,
    OpPub, //pub argument
    OpPubSpace,
    OpPubArg,
    OpMsg, //pub message
    OpMsgFull,
}
#[derive(Debug, PartialEq)]
struct SubArg<'a> {
    subject: &'a str, //为什么是str而不是String,就是为了避免内存分配,
    sid: &'a str,
    queue: Option<&'a str>,
}
#[derive(Debug, PartialEq)]
struct PubArg<'a> {
    subject: &'a str,
    size_buf: &'a str, // 1024 字符串形式,避免后续再次转换
    size: usize,       //1024 整数形式
    msg: &'a [u8],
}
#[derive(Debug, PartialEq)]
pub enum ParseResult<'a> {
    NoMsg, //buf="sub top.stevenbai.blog" sub消息不完整,我肯定不能处理
    Sub(SubArg<'a>),
    Pub(PubArg<'a>),
}
const BUF_LEN: usize = 512;
pub struct Parser {
    state: ParseState,
    buf: [u8; BUF_LEN], //消息解析缓冲区,如果消息不超过512,直接用这个,超过了就必须另分配
    arg_len: usize,
    msg_buf: Option<Vec<u8>>,
    //解析过程中受到新消息,那么 新消息的总长度是msg_total_len,已收到部分应该是msg_len
    msg_total_len: usize,
    msg_len: usize,
}

impl Parser {
    pub fn new() -> Self {
        Self {
            state: ParseState::OpStart,
            buf: [0; BUF_LEN],
            arg_len: 0,
            msg_buf: None,
            msg_total_len: 0,
            msg_len: 0,
        }
    }
    /**
    对收到的字节序列进行解析,解析完毕后得到pub或者sub消息,
    同时有可能没有消息或者缓冲区里面还有其他消息
    */
    pub fn parse<'a, 'b>(&'b mut self, buf: &'a [u8]) -> Result<(ParseResult<'a>, usize)>
    where
        'b: 'a,
    {
        let mut b;
        let mut i = 0;
        while i < buf.len() {
            use ParseState::*;
            b = buf[i] as char;
            //            println!("state={:?},b={}", self.state, b);
            match self.state {
                OpStart => match b {
                    'S' => self.state = OpS,
                    'P' => self.state = OpP,
                    _ => parse_error!(),
                },
                OpS => match b {
                    'U' => self.state = OpSu,
                    _ => parse_error!(),
                },
                OpSu => match b {
                    'B' => self.state = OpSub,
                    _ => parse_error!(),
                },
                OpSub => match b {
                    //sub stevenbai.top 3 是ok的,但是substevenbai.top 3就不允许
                    ' ' | '\t' => self.state = OPSubSpace,
                    _ => parse_error!(),
                },
                OPSubSpace => match b {
                    ' ' | '\t' => {}
                    _ => {
                        self.state = OpSubArg;
                        self.arg_len = 0;
                        i -= 1;
                    }
                },
                OpSubArg => match b {
                    '\r' => {}
                    '\n' => {
                        self.state = OpStart;
                        let r = self.process_sub()?;
                        return Ok((r, i + 1));
                    }
                    _ => {
                        self.add_arg(b as u8)?;
                    }
                },
                OpP => match b {
                    'U' => self.state = OpPu,
                    _ => parse_error!(),
                },
                OpPu => match b {
                    'B' => self.state = OpPub,
                    _ => parse_error!(),
                },
                OpPub => match b {
                    ' ' | '\t' => self.state = OpPubSpace,
                    _ => parse_error!(),
                },
                OpPubSpace => match b {
                    ' ' | '\t' => {}
                    _ => {
                        self.state = OpPubArg;
                        self.arg_len = 0;
                        i -= 1;
                    }
                },
                OpPubArg => match b {
                    '\r' => {}
                    '\n' => {
                        //PUB top.stevenbai 5\r\n
                        self.state = OpMsg;
                        let size = self.get_message_size()?;
                        if size == 0 || size > 1 * 1024 * 1024 {
                            //消息体长度不应该超过1M,防止Dos攻击
                            return Err(NError::new(ERROR_MESSAGE_SIZE_TOO_LARGE));
                        }
                        if size + self.arg_len > BUF_LEN {
                            self.msg_buf = Some(Vec::with_capacity(size));
                        }
                        self.msg_total_len = size;
                    }
                    _ => {
                        self.add_arg(b as u8)?;
                    }
                },
                OpMsg => {
                    //涉及消息长度
                    if self.msg_len < self.msg_total_len {
                        self.add_msg(b as u8);
                    } else {
                        self.state = OpMsgFull;
                    }
                }
                OpMsgFull => match b {
                    '\r' => {}
                    '\n' => {
                        self.state = OpStart;
                        let r = self.process_msg()?;
                        return Ok((r, i + 1));
                    }
                    _ => {
                        parse_error!();
                    }
                },
                _ => panic!("unkown state {:?}", self.state),
            }
            i += 1;
        }
        Ok((ParseResult::NoMsg, 0))
    }
    //一种是消息体比较短,可以直接放在buf中,无需另外分配内存
    //另一种是消息体很长,无法放在buf中,额外分配了msg_buf空间
    fn add_msg(&mut self, b: u8) {
        if let Some(buf) = self.msg_buf.as_mut() {
            buf.push(b);
        } else {
            //消息体比较短的情况
            if self.arg_len + self.msg_total_len > BUF_LEN {
                panic!("message should allocate space");
            }
            self.buf[self.arg_len + self.msg_len] = b;
        }
        self.msg_len += 1;
    }
    fn add_arg(&mut self, b: u8) -> Result<()> {
        //太长的subject
        if self.arg_len >= self.buf.len() {
            parse_error!();
        }
        self.buf[self.arg_len] = b;
        self.arg_len += 1;
        Ok(())
    }
    //解析缓冲区中的形如stevenbai.top queue 3
    fn process_sub(&self) -> Result<ParseResult> {
        let buf = &self.buf[0..self.arg_len];
        //有可能客户端恶意发送一些无效的utf8字符,这会导致错误.
        let ss = unsafe { std::str::from_utf8_unchecked(buf) };
        let mut arg_buf = [""; 3]; //如果没有queue,长度就是2,否则长度是3
        let mut arg_len = 0;
        for s in ss.split(' ') {
            if s.len() == 0 {
                continue;
            }
            if arg_len >= 3 {
                parse_error!();
            }
            arg_buf[arg_len] = s;
            arg_len += 1;
        }
        let mut sub_arg = SubArg {
            subject: "",
            sid: "",
            queue: None,
        };
        sub_arg.subject = arg_buf[0];
        //长度为2时不包含queue,为3包含queue,其他都说明格式错误
        match arg_len {
            2 => {
                sub_arg.sid = arg_buf[1];
            }
            3 => {
                sub_arg.sid = arg_buf[2];
                sub_arg.queue = Some(arg_buf[1]);
            }
            _ => parse_error!(),
        }
        Ok(ParseResult::Sub(sub_arg))
    }
    //解析缓冲区中以及msg_buf中的形如stevenbai.top 5hello
    fn process_msg(&self) -> Result<ParseResult> {
        let msg = if self.msg_buf.is_some() {
            self.msg_buf.as_ref().unwrap().as_slice()
        } else {
            &self.buf[self.arg_len..self.arg_len + self.msg_total_len]
        };
        let mut arg_buf = [""; 2];
        let mut arg_len = 0;
        let ss = unsafe { std::str::from_utf8_unchecked(&self.buf[0..self.arg_len]) };
        for s in ss.split(' ') {
            if s.len() == 0 {
                continue;
            }
            if arg_len >= 2 {
                parse_error!()
            }
            arg_buf[arg_len] = s;
            arg_len += 1;
        }
        let mut pub_arg = PubArg {
            subject: arg_buf[0],
            size_buf: arg_buf[1],
            size: self.msg_total_len,
            msg,
        };
        Ok(ParseResult::Pub(pub_arg))
    }
    //从接收到的pub消息中提前解析出来消息的长度
    fn get_message_size(&self) -> Result<usize> {
        //缓冲区中形如top.stevenbai.top 5
        let arg_buf = &self.buf[0..self.arg_len];
        let pos = arg_buf
            .iter()
            .rev()
            .position(|b| *b == ' ' as u8 || *b == '\t' as u8);
        if pos.is_none() {
            parse_error!();
        }
        let pos = pos.unwrap();
        let size_buf = &arg_buf[arg_buf.len() - pos..];
        let szb = unsafe { std::str::from_utf8_unchecked(size_buf) };
        szb.parse::<usize>().map_err(|_| NError::new(ERROR_PARSE))
    }
    pub fn iter<'a>(&'a mut self, buf: &'a [u8]) -> ParseIter<'a> {
        ParseIter { parser: self, buf }
    }
}
pub struct ParseIter<'a> {
    parser: *mut Parser,
    buf: &'a [u8],
}

impl<'a> Iterator for ParseIter<'a> {
    type Item = Result<ParseResult<'a>>;
    fn next(&mut self) -> Option<Self::Item> {
        if self.buf.len() == 0 {
            return None;
        }
        /*
        对于外部使用这类来说,这里使用unsafe是安全的.
        首先,ParseIter<'a>的生命周期一定是小于self.parser,也就是说parser这个指针一定是有效的.
        其次,ParseIter的构造只能通过Parser.iter来构造,所以parser一定是mutable的
        所以不存在内存安全问题.
        */
        let parser = unsafe { &mut *self.parser };
        let r: Result<(ParseResult<'a>, usize)> = parser.parse(self.buf);

        return Some(r.map(|r| {
            self.buf = &self.buf[r.1..];
            r.0
        }));
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn test() {}
    #[test]
    fn test_get_message_size() {
        let mut p = Parser::new();
        let buf = "subject 5".as_bytes();
        p.buf[0..buf.len()].copy_from_slice(buf);
        p.arg_len = buf.len();
        let r = p.get_message_size();
        assert!(r.is_ok());
        let r = r.unwrap();
        assert!(r == 5);
    }
    #[test]
    fn test_process_sub() {
        let mut p = Parser::new();
        let buf = "subject 5".as_bytes();
        p.buf[0..buf.len()].copy_from_slice(buf);
        p.arg_len = buf.len();
        let r = p.process_sub();
        assert!(r.is_ok());
        let r = r.unwrap();
        if let ParseResult::Sub(sub) = r {
            assert_eq!(sub.subject, "subject");
            assert_eq!(sub.sid, "5");
            assert!(sub.queue.is_none());
        } else {
            assert!(false, "unkown error");
        }
        //包含queue的情形
        let buf = "subject queue 5".as_bytes();
        p.buf[0..buf.len()].copy_from_slice(buf);
        p.arg_len = buf.len();
        let r = p.process_sub();
        assert!(r.is_ok());
        let r = r.unwrap();
        if let ParseResult::Sub(sub) = r {
            assert_eq!(sub.subject, "subject");
            assert_eq!(sub.sid, "5");
            assert_eq!(sub.queue.as_ref().unwrap(), &"queue");
        } else {
            assert!(false, "unkown error");
        }
    }
    #[test]
    fn test_process_pub() {
        let mut p = Parser::new();

        let buf = "subject 5hello".as_bytes();
        p.buf[0..buf.len()].copy_from_slice(buf);
        p.arg_len = buf.len() - 5;
        p.msg_total_len = 5;
        p.msg_len = 5;
        let r = p.process_msg();
        assert!(r.is_ok());
        let r = r.unwrap();
        if let ParseResult::Pub(pub_arg) = r {
            assert_eq!(pub_arg.subject, "subject");
            assert_eq!(pub_arg.size_buf, "5");
            assert_eq!(pub_arg.size, 5);
            assert_eq!(pub_arg.msg, "hello".as_bytes());
        } else {
            assert!(false, "unkown error");
        }
        let buf = "subject 5".as_bytes();
        p.buf[0..buf.len()].copy_from_slice(buf);
        p.arg_len = buf.len();
        p.msg_buf = Some(Vec::from("hello".as_bytes()));
        p.msg_total_len = 5;
        p.msg_len = 5;
        let r = p.process_msg();
        assert!(r.is_ok());
        let r = r.unwrap();
        if let ParseResult::Pub(pub_arg) = r {
            assert_eq!(pub_arg.subject, "subject");
            assert_eq!(pub_arg.size_buf, "5");
            assert_eq!(pub_arg.size, 5);
            assert_eq!(pub_arg.msg, "hello".as_bytes());
        } else {
            assert!(false, "unkown error");
        }
    }
    #[test]
    fn test_pub() {
        let mut p = Parser::new();
        assert!(p.parse("aa".as_bytes()).is_err());
        let buf = "PUB subject 5\r\nhello\r\n".as_bytes();
        let r = p.parse(buf);
        println!("r={:?}", r);
        assert!(r.is_ok());
        let r = r.unwrap();
        assert_eq!(r.1, buf.len());
        match r.0 {
            ParseResult::Pub(p) => {
                assert_eq!(p.subject, "subject");
                assert_eq!(p.size, 5);
                assert_eq!(p.size_buf, "5");
                assert_eq!(p.msg, "hello".as_bytes());
            }
            _ => assert!(false, "must be valid pub arg "),
        }
    }
    #[test]
    fn test_sub() {
        let mut p = Parser::new();
        let buf = "SUB subject 1\r\n".as_bytes();
        let r = p.parse(buf);
        assert!(r.is_ok());
        println!("r={:?}", r);
        let r = r.unwrap();
        assert_eq!(r.1, buf.len());
        if let ParseResult::Sub(sub) = r.0 {
            assert_eq!(sub.subject, "subject");
            assert_eq!(sub.sid, "1");
            assert_eq!(sub.queue, None);
        } else {
            assert!(false, "unkown error");
        }

        let buf = "SUB subject queue 1\r\n".as_bytes();
        let r = p.parse(buf);
        println!("r={:?}", r);
        assert!(r.is_ok());
        let r = r.unwrap();
        assert_eq!(r.1, buf.len());
        if let ParseResult::Sub(sub) = r.0 {
            assert_eq!(sub.subject, "subject");
            assert_eq!(sub.sid, "1");
            assert_eq!(sub.queue, Some("queue"));
        } else {
            assert!(false, "unkown error");
        }
    }
    #[test]
    fn test_sub2() {
        let mut p = Parser::new();
        let mut buf = "SUB subject 1\r\nSUB subject2 2\r\n".as_bytes();
        let mut pos = 0;
        loop {
            let r = p.parse(buf);
            assert!(!r.is_err());
            let r = r.unwrap();
            buf = &buf[r.1..];
            match r.0 {
                ParseResult::Sub(sub) => {
                    println!("sub.subect={}", sub.subject);
                }
                _ => panic!(),
            }
            if buf.len() == 0 {
                break;
            }
        }
    }
    #[test]
    fn test_sub3() {
        let mut p = Parser::new();
        let buf = "SUB subject 1\r\nSUB subject2 2\r\n".as_bytes();
        for r in p.iter(buf) {
            assert!(!r.is_err());
            let r = r.unwrap();
            match r {
                ParseResult::Sub(sub) => {
                    println!("sub.subect={}", sub.subject);
                }
                ParseResult::NoMsg => {
                    break;
                }
                _ => panic!(),
            }
        }
    }
    #[test]
    fn test_no_msg() {
        let mut p = Parser::new();
        let buf = "SUB subject".as_bytes();
        let r = p.parse(buf);
        assert!(r.is_ok());
        println!("r={:?}", r);
        let r = r.unwrap();
        assert_eq!(r.0, ParseResult::NoMsg);
    }
}
