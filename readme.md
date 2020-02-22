# 从零实现消息中间件

消息中间件在现代系统中非常关键,包括阿里云,腾讯云都有直接的消息中间件服务,也就是你不用自己搭建服务器,直接使用它提供的服务就可以了.那么我们今天就从零开始一步一步搭建一个极简消息中间件. 当然我们不可能做到像阿里云的RocketMQ那么复杂,但是最核心功能还是要保证的.

天实现的消息中间件系统不是基于MQTT,而是基于[nats](https://nats.io/),当然也是为了教学的方便,我们只会实现最核心的消息订阅发布,而围绕其的权限,cluster之类的我们都先屏蔽.对完整nats感兴趣的可以上[nats官网](https://nats.io/)查看完整的功能.

## 相关博客文章

- [1.从零实现一个极简消息中间件](从零实现一个极简消息中间件)
- [2.parser](从零实现消息中间件-parser)
- [3.sublist](从零实现消息中间件-sublist)
- [4.server](从零实现消息中间件-server)
- [5.server-client](从零实现消息中间件-server.client)
- [6.client library](从零实现消息中间件-client)


## B站相关视频

- [1.parser](https://www.bilibili.com/video/av85936685)
- [2.sublist](https://www.bilibili.com/video/av86899713/)
- [3.server](https://www.bilibili.com/video/av90457400/)
- [4.server-client](https://www.bilibili.com/video/av90457552/)
- [5.client library](https://www.bilibili.com/video/av90458399/)

## 协议设计
nats是一个文本格式的通信协议,本来就非常简单,加上我们这次教学的需要,只保留了最核心的订阅发布系统.那就更简单了. 消息总共只有三种(订阅,发布,消息推送). 
为了简化实现,就不支持取消订阅功能,如果想取消订阅,只能断开连接了.
### 订阅主题
所谓订阅,首先是要订阅什么. nats中的主题是类似于域名格式,形如top.stevenbai.blog. 比如我订阅了top.stevenbai.blog,那么当有人在这个主题下发布消息的时候我就收的到.
当然为了使用的方便,我们还支持主题的模糊匹配,具体来说就是*和>.
#### *匹配
*只匹配.分割的一个字段.
比如top.\*.blog 则可以匹配top.stevenbai.blog,top.steven.blog等等
而top.\*,则可以匹配top.stevenbai,top.steven,但是不能匹配top.stevenbai.blog.
#### >匹配
`>`可以匹配所有的字段.
比如top.> 则可以匹配包括top.stevenbai,top.stevenbai.blog,top.steven.blog等
一般来说调试的时候我们可以订阅这么一个主题`>`,他会匹配所有的主题,也就是说所有人发布的消息都可以收到.
当然只是调试的时候,因为真实的生产环境中这么使用,很快这个client就会被淹没.

### 发布消息(PUB)
```
PUB <subject> <size>\r\n
<message>\r\n
```
发布消息格式很简单,就是我想在某个subject下发布一个长度为多少的消息,这个消息可以使纯文本,也可以是二进制.

### 订阅消息(SUB)
```
SUB <subject> <sid>\r\n
```
具体来说就是表达对某个subject感兴趣,如果有人在这个subject下发布了消息,那么请推送给我.推送的格式见消息推送. 
其中sid是对订阅的编号,是一个十进制整数. 因为同一个tcp连接是可以有任意多个订阅.

#### 负载均衡
同一subject的消息发布方可能有很多个,比如一个物联网系统中,同一类型的设备都会在某个主题下发布消息. 而这个消息可能每秒钟有上百万条,这时候一个接收方肯定就忙不过来了. 这时候就可以多个接收方.
因此从设计角度来说nats的消息订阅发布系统是多对多的. 也就是说一个主题下可以有多个发送发,多个接收方. 
带负载均衡的订阅:
```
SUB <subject> <queue> <sid>
```
比如两个client,clientA和B分别订阅了`sub top.stevenbai.blog workers 3`和`sub top.stevenbai.blog workers 4`.这里的3和4分别是两个连接各自的订阅id,他们没有任何关系,可以相同也可以不同,是他们自己的安排.
如果这时有一个client C发布了`pub top.stevenbai.blog 5\r\nfirst`和`pub top.stevenbai.blog 6\r\nsecond`两条消息,则A和B将分别收到`first`和`second`.
### 消息推送
订阅发布消息都是客户端向服务器发出,而消息推送则是服务器向客户端发出. 格式如下:
```
MSG <subject> <sid> <size>\r\n
<message>\r\n
```
这个格式看起来和pub消息的非常像,只不过关键字是MSG,而且多了一个<sid>表示这个连接上的订阅编号.
举例来说,上面的例子client C发布了`pub top.stevenbai.blog 5\r\nfirst`,那么ClientA收到的消息格式就是
```
MSG top.stevenbai.blog 3 5\r\n
first\r\n
```

## 系统设计
根据上面的协议设计.
### 客户端的一般工作流程.
#### 消息订阅方的工作流程
1. 建立一个tcp连接
2. sub一个或者多个主题
3. 等等相关消息
#### 消息发布方的工作流程
1. 建立一个tcp连接
2. 重复的在一个或者多个主题下pub消息
客户端的工作看了起来非常直观.

### 服务端的工作流程
#### 消息格式解析
目前就两种消息pub和sub.
#### 主题的树状组织
trie树,是一种字典树 a.b.c

按照前面的描述当客户端在一个主题下pub消息的时候,服务器要能找到所有对这个主题感兴趣的客户端,因为要支持*和>的模糊匹配,使用trie树来组织比较合理.

明显这里的trie树是系统的核心数据,每一次client的pub都要来这里查找所有相关的sub,如果这里设计的不好肯定会造成系统的瓶颈.
1. 这颗trie树是全局的,每一次新的订阅和连接的断开都需要更新
2. 每一次pub都需要在树中查找.
所以树的访问必须带锁;为了避免重复查找,要进行cache.

#### client的管理
这是所有server都要做的,这也是这个系统的核心部分.
1. 计划使用tokio 0.2
2. trie树的管理
3. client的管理,新建连接,连接断开等.



https://github.com/nkbai/learnrustbynats

https://github.com/nkbai/tokio/tree/readcode
