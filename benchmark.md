before any optimaztion: debug server+client

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



before any optimaztion: release server+client 
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



## 使用BytesMut优化缓存,减少await调用,就有六倍的提升.
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
## 使用多个pub,测试,代码和刚刚的一样,
性能有10%以上的提升,但是观察到nats-server cpu占用一致比较低,没有超过client
   cargo run  --release  -- --subject test --num-subs 10 --num-msgs 1000000 --num-pubs 10 

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

