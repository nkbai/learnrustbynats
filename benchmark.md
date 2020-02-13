before any optimaztion: debug server+client
``` 

Nats pub/sub stats: 28507 msgs/sec ~ 3649009.168600655/sec
  Pub stats: 2770 msgs/sec ~ 354636.4610452923/sec
  Sub stats: 25916 msgs/sec ~ 3317281.062364232/sec
   [1] 2591 msgs/sec ~ 331732.67399158835/sec (100000 msgs)
   [2] 2591 msgs/sec ~ 331737.8092221816/sec (100000 msgs)
   [3] 2591 msgs/sec ~ 331740.81480018055/sec (100000 msgs)
   [4] 2591 msgs/sec ~ 331738.8905870689/sec (100000 msgs)
   [5] 2591 msgs/sec ~ 331742.6321808266/sec (100000 msgs)
   [6] 2591 msgs/sec ~ 331731.7214811043/sec (100000 msgs)
   [7] 2591 msgs/sec ~ 331743.00491758913/sec (100000 msgs)
   [8] 2591 msgs/sec ~ 331732.89617403643/sec (100000 msgs)
   [9] 2591 msgs/sec ~ 331739.76043689373/sec (100000 msgs)
   [10] 2591 msgs/sec ~ 331744.08291154966/sec (100000 msgs)
   min 2591 | avg 2591 | max  2591 | stddev 0 msgs

```


before any optimaztion: release server+client 
``` 
Nats pub/sub stats: 61389 msgs/sec ~ 7857896.78813678/sec
  Pub stats: 5882 msgs/sec ~ 752915.7634376923/sec
  Sub stats: 55808 msgs/sec ~ 7143542.5346698/sec
   [1] 5580 msgs/sec ~ 714362.6022647332/sec (100000 msgs)
   [2] 5581 msgs/sec ~ 714379.9072697352/sec (100000 msgs)
   [3] 5581 msgs/sec ~ 714383.2669884142/sec (100000 msgs)
   [4] 5581 msgs/sec ~ 714392.3734657292/sec (100000 msgs)
   [5] 5581 msgs/sec ~ 714388.2732535391/sec (100000 msgs)
   [6] 5581 msgs/sec ~ 714394.9112222577/sec (100000 msgs)
   [7] 5581 msgs/sec ~ 714398.9512635452/sec (100000 msgs)
   [8] 5580 msgs/sec ~ 714365.3115635442/sec (100000 msgs)
   [9] 5581 msgs/sec ~ 714368.6850990714/sec (100000 msgs)
   [10] 5581 msgs/sec ~ 714399.5103139627/sec (100000 msgs)
   min 5580 | avg 5580 | max  5581 | stddev 0.8944271909999159 msgs
```


## 使用BytesMut优化缓存,减少await调用,就有六倍的提升.
```
Nats pub/sub stats: 377984 msgs/sec ~ 48382031.15402761/sec
  Pub stats: 36107 msgs/sec ~ 4621775.446487484/sec
  Sub stats: 343622 msgs/sec ~ 43983664.68547965/sec
   [1] 34366 msgs/sec ~ 4398952.634534228/sec (100000 msgs)
   [2] 34375 msgs/sec ~ 4400015.705993563/sec (100000 msgs)
   [3] 34378 msgs/sec ~ 4400465.716413369/sec (100000 msgs)
   [4] 34363 msgs/sec ~ 4398490.865962944/sec (100000 msgs)
   [5] 34376 msgs/sec ~ 4400181.333672851/sec (100000 msgs)
   [6] 34370 msgs/sec ~ 4399367.908305072/sec (100000 msgs)
   [7] 34368 msgs/sec ~ 4399154.384783533/sec (100000 msgs)
   [8] 34372 msgs/sec ~ 4399698.322447861/sec (100000 msgs)
   [9] 34363 msgs/sec ~ 4398570.699846082/sec (100000 msgs)
   [10] 34370 msgs/sec ~ 4399408.850882781/sec (100000 msgs)
   min 34363 | avg 34370 | max  34378 | stddev 4.969909455915671 msgs
```

## 使用多个pub,测试,代码和刚刚的一样,

性能有10%以上的提升,但是观察到nats-server cpu占用一致比较低,没有超过client
   cargo run  --release  -- --subject test --num-subs 10 --num-msgs 1000000 --num-pubs 10 
```
   Nats pub/sub stats: 433364 msgs/sec ~ 55470702.813337795/sec
  Pub stats: 41424 msgs/sec ~ 5302354.359580678/sec
   [1] 4149 msgs/sec ~ 531126.7721818777/sec (100000 msgs)
   [2] 4149 msgs/sec ~ 531104.6321667176/sec (100000 msgs)
   [3] 4148 msgs/sec ~ 530950.1965164108/sec (100000 msgs)
   [4] 4148 msgs/sec ~ 530952.9439894602/sec (100000 msgs)
   [5] 4149 msgs/sec ~ 531135.9464654801/sec (100000 msgs)
   [6] 4147 msgs/sec ~ 530886.8992623787/sec (100000 msgs)
   [7] 4148 msgs/sec ~ 530979.3590014324/sec (100000 msgs)
   [8] 4148 msgs/sec ~ 530954.9749239235/sec (100000 msgs)
   [9] 4146 msgs/sec ~ 530720.1580503538/sec (100000 msgs)
   [10] 4146 msgs/sec ~ 530730.5288312284/sec (100000 msgs)
  Sub stats: 393968 msgs/sec ~ 50427911.6484889/sec
   [1] 39397 msgs/sec ~ 5042943.996870369/sec (1000000 msgs)
   [2] 39398 msgs/sec ~ 5042977.890046073/sec (1000000 msgs)
   [3] 39399 msgs/sec ~ 5043108.388063024/sec (1000000 msgs)
   [4] 39398 msgs/sec ~ 5043044.403963505/sec (1000000 msgs)
   [5] 39398 msgs/sec ~ 5043000.823317792/sec (1000000 msgs)
   [6] 39397 msgs/sec ~ 5042888.875959185/sec (1000000 msgs)
   [7] 39397 msgs/sec ~ 5042860.738008805/sec (1000000 msgs)
   [8] 39399 msgs/sec ~ 5043073.080630799/sec (1000000 msgs)
   [9] 39396 msgs/sec ~ 5042811.453494481/sec (1000000 msgs)
   [10] 39396 msgs/sec ~ 5042791.16484889/sec (1000000 msgs)
   min 39396 | avg 39397 | max  39399 | stddev 1.140175425099138 msgs
```

## 优化,msg_buf不再每次都分配
但是这里有一个问题,这个buf什么时候释放呢?一直占着?并且不会缩小.
只要连接不断开,就会一直占用.
经测试影响不大,因为根本就没有用到msg_buf,目前的msg_size是128,不会用到.
```
Nats pub/sub stats: 377535 msgs/sec ~ 48324568.42183404/sec
  Pub stats: 36157 msgs/sec ~ 4628158.023356056/sec
  Sub stats: 343214 msgs/sec ~ 43931425.83803094/sec
   [1] 34326 msgs/sec ~ 4393757.401522481/sec (100000 msgs)
   [2] 34324 msgs/sec ~ 4393530.965829699/sec (100000 msgs)
   [3] 34327 msgs/sec ~ 4393877.165810065/sec (100000 msgs)
   [4] 34330 msgs/sec ~ 4394268.074840311/sec (100000 msgs)
   [5] 34328 msgs/sec ~ 4394076.038781426/sec (100000 msgs)
   [6] 34333 msgs/sec ~ 4394714.7405391075/sec (100000 msgs)
   [7] 34335 msgs/sec ~ 4394895.1331806155/sec (100000 msgs)
   [8] 34332 msgs/sec ~ 4394508.732300149/sec (100000 msgs)
   [9] 34321 msgs/sec ~ 4393189.280530978/sec (100000 msgs)
   [10] 34330 msgs/sec ~ 4394268.596802779/sec (100000 msgs)
   min 34321 | avg 34328 | max  34335 | stddev 4.09878030638384 msgs

Nats pub/sub stats: 385345 msgs/sec ~ 49324272.01149149/sec
  Pub stats: 36887 msgs/sec ~ 4721574.6730107395/sec
  Sub stats: 350314 msgs/sec ~ 44840247.28317408/sec
   [1] 35047 msgs/sec ~ 4486101.932145457/sec (100000 msgs)
   [2] 35043 msgs/sec ~ 4485538.845636878/sec (100000 msgs)
   [3] 35049 msgs/sec ~ 4486307.3576117465/sec (100000 msgs)
   [4] 35038 msgs/sec ~ 4484933.827490584/sec (100000 msgs)
   [5] 35044 msgs/sec ~ 4485645.19838767/sec (100000 msgs)
   [6] 35050 msgs/sec ~ 4486476.823528619/sec (100000 msgs)
   [7] 35051 msgs/sec ~ 4486560.542245931/sec (100000 msgs)
   [8] 35053 msgs/sec ~ 4486888.8000169415/sec (100000 msgs)
   [9] 35034 msgs/sec ~ 4484385.156475487/sec (100000 msgs)
   [10] 35031 msgs/sec ~ 4484024.728317408/sec (100000 msgs)
   min 35031 | avg 35044 | max  35053 | stddev 7.113367697511496 msgs
```

## 并发发送
send_message由串行改为并发,
`--num-pubs 1`基本没变化,但是改为`--num-pubs 10`有一倍提升.
 这种情况下，无论是client还是server内存占用都非常低。 考虑启用缓存,来空间换时间.

### --num-pubs 1
```
Nats pub/sub stats: 376232 msgs/sec ~ 48157765.32076849/sec
  Pub stats: 34386 msgs/sec ~ 4401420.72194075/sec
  Sub stats: 342029 msgs/sec ~ 43779786.65524408/sec
   [1] 34205 msgs/sec ~ 4378252.77514042/sec (1000000 msgs)
   [2] 34204 msgs/sec ~ 4378216.303690606/sec (1000000 msgs)
   [3] 34203 msgs/sec ~ 4378102.962688762/sec (1000000 msgs)
   [4] 34204 msgs/sec ~ 4378131.4951717425/sec (1000000 msgs)
   [5] 34205 msgs/sec ~ 4378278.877766064/sec (1000000 msgs)
   [6] 34204 msgs/sec ~ 4378176.879110729/sec (1000000 msgs)
   [7] 34203 msgs/sec ~ 4377995.095039316/sec (1000000 msgs)
   [8] 34203 msgs/sec ~ 4378071.547512658/sec (1000000 msgs)
   [9] 34203 msgs/sec ~ 4378032.975034622/sec (1000000 msgs)
   [10] 34204 msgs/sec ~ 4378138.376806764/sec (1000000 msgs)
   min 34203 | avg 34203 | max  34205 | stddev 1.0954451150103321 msgs
```
### --num-pubs 10
```
Nats pub/sub stats: 790452 msgs/sec ~ 101177900.76085752/sec
  Pub stats: 75425 msgs/sec ~ 9654401.307397576/sec
   [1] 7579 msgs/sec ~ 970234.0958495921/sec (100000 msgs)
   [2] 7574 msgs/sec ~ 969559.4115457184/sec (100000 msgs)
   [3] 7572 msgs/sec ~ 969224.644279701/sec (100000 msgs)
   [4] 7572 msgs/sec ~ 969298.5754256473/sec (100000 msgs)
   [5] 7572 msgs/sec ~ 969326.885318488/sec (100000 msgs)
   [6] 7573 msgs/sec ~ 969461.2974012174/sec (100000 msgs)
   [7] 7570 msgs/sec ~ 968976.6409039636/sec (100000 msgs)
   [8] 7571 msgs/sec ~ 969158.8835072055/sec (100000 msgs)
   [9] 7573 msgs/sec ~ 969358.9971748831/sec (100000 msgs)
   [10] 7569 msgs/sec ~ 968873.5004742752/sec (100000 msgs)
  Sub stats: 718593 msgs/sec ~ 91979909.78259775/sec
   [1] 71862 msgs/sec ~ 9198385.281672334/sec (1000000 msgs)
   [2] 71866 msgs/sec ~ 9198897.934254501/sec (1000000 msgs)
   [3] 71863 msgs/sec ~ 9198491.500543945/sec (1000000 msgs)
   [4] 71859 msgs/sec ~ 9198034.709645862/sec (1000000 msgs)
   [5] 71864 msgs/sec ~ 9198643.851382144/sec (1000000 msgs)
   [6] 71865 msgs/sec ~ 9198771.76335013/sec (1000000 msgs)
   [7] 71862 msgs/sec ~ 9198411.781956475/sec (1000000 msgs)
   [8] 71861 msgs/sec ~ 9198245.238568164/sec (1000000 msgs)
   [9] 71860 msgs/sec ~ 9198140.883406043/sec (1000000 msgs)
   [10] 71863 msgs/sec ~ 9198546.939085985/sec (1000000 msgs)
   min 71859 | avg 71862 | max  71866 | stddev 2.1213203435596424 msgs
```


## 加入本地cache
只是为了测试,没有更新机制
不完善,可以看到有一定提高,但是不明显,几个百分点的样子
### -pubs 1
```
Nats pub/sub stats: 390384 msgs/sec ~ 49969156.719011426/sec
  Pub stats: 35743 msgs/sec ~ 4575136.18287007/sec
  Sub stats: 354894 msgs/sec ~ 45426506.108192205/sec
   [1] 35490 msgs/sec ~ 4542791.626176854/sec (1000000 msgs)
   [2] 35489 msgs/sec ~ 4542685.696734685/sec (1000000 msgs)
   [3] 35490 msgs/sec ~ 4542804.556233796/sec (1000000 msgs)
   [4] 35490 msgs/sec ~ 4542841.2181290435/sec (1000000 msgs)
   [5] 35491 msgs/sec ~ 4542859.491013554/sec (1000000 msgs)
   [6] 35489 msgs/sec ~ 4542700.385740192/sec (1000000 msgs)
   [7] 35489 msgs/sec ~ 4542717.684684231/sec (1000000 msgs)
   [8] 35490 msgs/sec ~ 4542745.48128393/sec (1000000 msgs)
   [9] 35490 msgs/sec ~ 4542815.080036936/sec (1000000 msgs)
   [10] 35490 msgs/sec ~ 4542788.906937107/sec (1000000 msgs)
   min 35489 | avg 35489 | max  35491 | stddev 1 msgs
```
### -pubs 10
```
   Nats pub/sub stats: 813669 msgs/sec ~ 104149682.70495887/sec
  Pub stats: 77845 msgs/sec ~ 9964231.962461589/sec
   [1] 7821 msgs/sec ~ 1001095.0332549638/sec (100000 msgs)
   [2] 7812 msgs/sec ~ 1000051.0897194011/sec (100000 msgs)
   [3] 7811 msgs/sec ~ 999847.6086172981/sec (100000 msgs)
   [4] 7810 msgs/sec ~ 999774.9932175713/sec (100000 msgs)
   [5] 7815 msgs/sec ~ 1000337.8641917821/sec (100000 msgs)
   [6] 7807 msgs/sec ~ 999348.5271154706/sec (100000 msgs)
   [7] 7805 msgs/sec ~ 999100.0203804227/sec (100000 msgs)
   [8] 7809 msgs/sec ~ 999660.2638192733/sec (100000 msgs)
   [9] 7805 msgs/sec ~ 999132.7749196282/sec (100000 msgs)
   [10] 7806 msgs/sec ~ 999202.2221919566/sec (100000 msgs)
  Sub stats: 739699 msgs/sec ~ 94681529.7317808/sec
   [1] 73976 msgs/sec ~ 9469015.62481706/sec (1000000 msgs)
   [2] 73975 msgs/sec ~ 9468881.113728898/sec (1000000 msgs)
   [3] 73974 msgs/sec ~ 9468771.64819417/sec (1000000 msgs)
   [4] 73979 msgs/sec ~ 9469414.929399932/sec (1000000 msgs)
   [5] 73978 msgs/sec ~ 9469243.13565144/sec (1000000 msgs)
   [6] 73977 msgs/sec ~ 9469088.617461707/sec (1000000 msgs)
   [7] 73970 msgs/sec ~ 9468177.11740739/sec (1000000 msgs)
   [8] 73973 msgs/sec ~ 9468598.682160331/sec (1000000 msgs)
   [9] 73971 msgs/sec ~ 9468336.636719728/sec (1000000 msgs)
   [10] 73972 msgs/sec ~ 9468442.361155936/sec (1000000 msgs)
   min 73970 | avg 73974 | max  73979 | stddev 2.9154759474226504 msgs
```   
## 客户端服务端批量消息处理
在测试过程中发现,单客户端批量消息发送,效果并不理想,两者结合起来,则有大幅度提升
可以听到到2500000 msgs/sec

### -pub 1
```
Nats pub/sub stats: 2820266 msgs/sec ~ 360994066.07799476/sec
  Pub stats: 272533 msgs/sec ~ 34884233.19733797/sec
  Sub stats: 2563878 msgs/sec ~ 328176423.70726794/sec
   [1] 259650 msgs/sec ~ 33235314.85993536/sec (100000 msgs)
   [2] 259853 msgs/sec ~ 33261248.67171009/sec (100000 msgs)
   [3] 258512 msgs/sec ~ 33089597.940031897/sec (100000 msgs)
   [4] 258564 msgs/sec ~ 33096266.07228048/sec (100000 msgs)
   [5] 258607 msgs/sec ~ 33101786.509107266/sec (100000 msgs)
   [6] 258163 msgs/sec ~ 33044888.545773942/sec (100000 msgs)
   [7] 258153 msgs/sec ~ 33043639.743700314/sec (100000 msgs)
   [8] 256826 msgs/sec ~ 32873816.19622194/sec (100000 msgs)
   [9] 257215 msgs/sec ~ 32923624.123998497/sec (100000 msgs)
   [10] 257401 msgs/sec ~ 32947340.651538294/sec (100000 msgs)
   min 256826 | avg 258294 | max  259853 | stddev 929.5894792864214 msgs
```

### --num-pubs 10 
没有明显变化,和`--num-pubs 1`是差不多的
```
Nats pub/sub stats: 2836183 msgs/sec ~ 363031456.8459067/sec
  Pub stats: 455987 msgs/sec ~ 58366353.48983736/sec
   [1] 46964 msgs/sec ~ 6011486.664181387/sec (10000 msgs)
   [2] 47609 msgs/sec ~ 6093961.236245924/sec (10000 msgs)
   [3] 46671 msgs/sec ~ 5973945.188782198/sec (10000 msgs)
   [4] 46761 msgs/sec ~ 5985441.694337137/sec (10000 msgs)
   [5] 46871 msgs/sec ~ 5999539.835294633/sec (10000 msgs)
   [6] 46971 msgs/sec ~ 6012344.414030855/sec (10000 msgs)
   [7] 46890 msgs/sec ~ 6002020.064256032/sec (10000 msgs)
   [8] 47038 msgs/sec ~ 6020924.7738744095/sec (10000 msgs)
   [9] 47144 msgs/sec ~ 6034558.284076369/sec (10000 msgs)
   [10] 45624 msgs/sec ~ 5839918.179461352/sec (10000 msgs)
  Sub stats: 2578348 msgs/sec ~ 330028597.13264245/sec
   [1] 267107 msgs/sec ~ 34189728.241582826/sec (100000 msgs)
   [2] 265067 msgs/sec ~ 33928665.84947354/sec (100000 msgs)
   [3] 264266 msgs/sec ~ 33826095.06122654/sec (100000 msgs)
   [4] 264055 msgs/sec ~ 33799086.41650338/sec (100000 msgs)
   [5] 262109 msgs/sec ~ 33549999.75597617/sec (100000 msgs)
   [6] 262331 msgs/sec ~ 33578437.388171345/sec (100000 msgs)
   [7] 260781 msgs/sec ~ 33380085.707386095/sec (100000 msgs)
   [8] 259417 msgs/sec ~ 33205397.433585964/sec (100000 msgs)
   [9] 259557 msgs/sec ~ 33223381.159022104/sec (100000 msgs)
   [10] 258487 msgs/sec ~ 33086353.612291187/sec (100000 msgs)
   min 258487 | avg 262317 | max  267107 | stddev 2653.772804894948 msgs

```