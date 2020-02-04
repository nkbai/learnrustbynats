/**

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
    OpM,
    OpMs,
    OpMsg,
    OpMsgSpc,
    OpMsgArg,
    OpMsgBody, //pub message
    OpMsgFull,
}

#[derive(Debug, PartialEq)]
pub struct MsgArg<'a> {
    pub subject: &'a str,
    pub size: usize, //1024 整数形式
    pub sid: &'a str,
    pub msg: &'a [u8],
}
#[derive(Debug, PartialEq)]
pub enum ParseResult<'a> {
    NoMsg, //buf="sub top.stevenbai.blog" sub消息不完整,我肯定不能处理
    MsgArg(MsgArg<'a>),
}
/*
这个长度很有关系,必须能够将一个完整的主题以及参数放进去,
所以要限制subject的长度
*/
const BUF_LEN: usize = 512;
pub struct Parser {
    state: ParseState,
    buf: [u8; BUF_LEN], //消息解析缓冲区,如果消息不超过512,直接用这个,超过了就必须另分配
    arg_len: usize,
    msg_buf: Option<Vec<u8>>,
    //解析过程中受到新消息,那么 新消息的总长度是msg_total_len,已收到部分应该是msg_len
    msg_total_len: usize,
    msg_len: usize,
    debug: bool,
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
            debug: true,
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
        if self.debug {
            println!(
                "parse string:{},state={:?}",
                unsafe { std::str::from_utf8_unchecked(buf) },
                self.state
            );
        }
        while i < buf.len() {
            use ParseState::*;
            b = buf[i] as char;
            //            println!("state={:?},b={}", self.state, b);
            match self.state {
                OpStart => match b {
                    'M' => self.state = OpM,
                    _ => parse_error!(),
                },
                OpM => match b {
                    'S' => self.state = OpMs,
                    _ => parse_error!(),
                },
                OpMs => match b {
                    'G' => self.state = OpMsg,
                    _ => parse_error!(),
                },
                OpMsg => match b {
                    ' ' | '\t' => self.state = OpMsgSpc,
                    _ => parse_error!(),
                },
                OpMsgSpc => match b {
                    ' ' | '\t' => {}
                    _ => {
                        self.state = OpMsgArg;
                        self.arg_len = 0;
                        continue;
                    }
                },
                OpMsgArg => match b {
                    '\r' => {}
                    '\n' => {
                        self.state = OpMsgBody;
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
                OpMsgBody => {
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

    //解析缓冲区中以及msg_buf中的形如stevenbai.top 5hello
    fn process_msg(&self) -> Result<ParseResult> {
        let msg = if self.msg_buf.is_some() {
            self.msg_buf.as_ref().unwrap().as_slice()
        } else {
            &self.buf[self.arg_len..self.arg_len + self.msg_total_len]
        };
        let mut arg_buf = [""; 3];
        let mut arg_len = 0;
        let ss = unsafe { std::str::from_utf8_unchecked(&self.buf[0..self.arg_len]) };
        for s in ss.split(' ') {
            if s.len() == 0 {
                continue;
            }
            if arg_len >= 3 {
                parse_error!()
            }
            arg_buf[arg_len] = s;
            arg_len += 1;
        }
        let mut msg_arg = MsgArg {
            subject: arg_buf[0],
            size: self.msg_total_len,
            sid: arg_buf[1],
            msg,
        };
        Ok(ParseResult::MsgArg(msg_arg))
    }
    pub fn clear_msg_buf(&mut self) {
        self.msg_buf = None;
        self.msg_len = 0;
        self.msg_total_len = 0;
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
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_msg() {
        let mut p = Parser::new();
        assert!(p.parse("aa".as_bytes()).is_err());
        let mut buf = "MSG subject 1 5\r\nhello\r\nMSG subject 1 5\r\nxxxxx\r\n".as_bytes();
        let r = p.parse(buf);
        println!("r={:?}", r);
        assert!(r.is_ok());
        let (r, n) = r.unwrap();
        //        assert_eq!(r.1, buf.len());
        match r {
            ParseResult::MsgArg(p) => {
                assert_eq!(p.subject, "subject");
                assert_eq!(p.size, 5);
                assert_eq!(p.msg, "hello".as_bytes());
            }
            _ => assert!(false, "must be valid pub arg "),
        }
        p.clear_msg_buf();
        if n < buf.len() {
            buf = &buf[n..];
            let r = p.parse(buf);
            println!("r={:?}", r);
            assert!(r.is_ok());
            let r = r.unwrap();
            //        assert_eq!(r.1, buf.len());
            match r.0 {
                ParseResult::MsgArg(p) => {
                    assert_eq!(p.subject, "subject");
                    assert_eq!(p.size, 5);
                    assert_eq!(p.msg, "xxxxx".as_bytes());
                }
                _ => assert!(false, "must be valid pub arg "),
            }
        }
    }
}
