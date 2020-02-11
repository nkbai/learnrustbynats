use std::cmp::max;
use std::fmt::{Error, Formatter};

use bytes::buf::BufMutExt;
use bytes::BytesMut;
use std::cmp::min;
use std::fmt::Write;
use std::ops::Sub;
use tokio::time::{Duration, Instant};

#[derive(Debug, Clone)]
pub struct Sample {
    job_msg_cnt: usize,
    msg_count: u64,
    msg_bytes: u64,
    io_bytes: u64,
    start: Instant,
    end: Instant,
}
impl Default for Sample {
    fn default() -> Self {
        Self {
            job_msg_cnt: 0,
            msg_count: 0,
            msg_bytes: 0,
            io_bytes: 0,
            start: Instant::now() + Duration::from_secs(60 * 60 * 24),
            end: Instant::now() - Duration::from_secs(60 * 60 * 24),
        }
    }
}
impl Sample {
    pub fn new(
        job_count: usize,
        msg_size: usize,
        msg_count: u64, //发出和收到的消息总和
        io_bytes: u64,  //发出和收到的字节数总和
        start: Instant,
        end: Instant,
    ) -> Self {
        Self {
            job_msg_cnt: job_count,
            msg_count,
            msg_bytes: (msg_size * job_count) as u64,
            io_bytes,
            start,
            end,
        }
    }
    // Throughput of bytes per second
    pub fn throughput(&self) -> f64 {
        self.msg_bytes as f64 / self.duration().as_secs_f64()
    }

    pub fn duration(&self) -> Duration {
        self.end.sub(self.start)
    }
    // Rate of meessages in the job per second
    pub fn rate(&self) -> i64 {
        (self.job_msg_cnt as f64 / self.duration().as_secs_f64()) as i64
    }
    pub fn add_statistics(&mut self, other: &Self) {
        self.msg_count += other.msg_count;
        self.job_msg_cnt += other.job_msg_cnt;
        self.io_bytes += other.io_bytes;
        self.msg_bytes += other.msg_bytes;
        if self.start > other.start {
            self.start = other.start;
        }
        if self.end < other.end {
            self.end = other.end
        }
    }
}
impl std::fmt::Display for Sample {
    fn fmt(&self, f: &mut Formatter<'_>) -> Result<(), Error> {
        let rate = self.rate();
        let throughput = self.throughput();
        write!(f, "{} msgs/sec ~ {}/sec", rate, throughput)
    }
}
#[derive(Debug, Default, Clone)]
struct SampleGroup {
    group_sample: Sample,
    samples: Vec<Sample>,
}
impl SampleGroup {
    // AddSample adds a Sample to the SampleGroup. After adding a Sample it shouldn't be modified.
    pub fn add_sample(&mut self, s: Sample) {
        if self.samples.is_empty() {
            self.group_sample.start = s.start;
            self.group_sample.end = s.end;
        }
        self.group_sample.add_statistics(&s);
        if s.start < self.group_sample.start {
            self.group_sample.start = s.start;
        }
        if s.end > self.group_sample.end {
            self.group_sample.end = s.end;
        }
        self.samples.push(s)
    }

    pub fn has_samples(&self) -> bool {
        !self.samples.is_empty()
    }
    pub fn statistics(&self) -> String {
        format!(
            "min {} | avg {} | max  {} | stddev {} msgs",
            self.min_rate(),
            self.avg_rate(),
            self.max_rate(),
            self.std_dev(),
        )
    }
    // MinRate returns the smallest message rate in the SampleGroup
    pub fn min_rate(&self) -> i64 {
        let mut m = std::i64::MAX;
        for s in self.samples.iter() {
            m = min(m, s.rate());
        }
        m
    }
    pub fn max_rate(&self) -> i64 {
        let mut m = std::i64::MIN;
        for s in self.samples.iter() {
            m = max(m, s.rate());
        }
        m
    }
    pub fn avg_rate(&self) -> i64 {
        if self.samples.is_empty() {
            return 0;
        }
        let mut sum = 0;
        for s in self.samples.iter() {
            sum += s.rate();
        }
        sum / self.samples.len() as i64
    }
    // StdDev returns the standard deviation the message rates in the SampleGroup
    //求速率的标准差
    pub fn std_dev(&self) -> f64 {
        if self.samples.is_empty() {
            return 0.0;
        }
        let avg = self.avg_rate() as f64;
        let mut sum = 0 as f64;
        for s in self.samples.iter() {
            sum += (s.rate() as f64 - avg).powf(2.0);
        }
        let variance = sum / self.samples.len() as f64;
        variance.sqrt()
    }
}

#[derive(Debug, Default, Clone)]
pub struct Benchmark {
    bench_sample: Sample,
    name: String,
    pubs: SampleGroup,
    subs: SampleGroup,
}

impl Benchmark {
    pub fn new<S: Into<String>>(name: S) -> Self {
        Self {
            bench_sample: Default::default(),
            name: name.into(),
            pubs: Default::default(),
            subs: Default::default(),
        }
    }
    pub fn add_pub_sample(&mut self, s: Sample) {
        self.bench_sample.add_statistics(&s);
        self.pubs.add_sample(s);
    }
    pub fn add_sub_sample(&mut self, s: Sample) {
        self.bench_sample.add_statistics(&s);
        self.subs.add_sample(s);
    }
    // Report returns a human readable report of the samples taken in the Benchmark
    pub fn report(&self) -> String {
        let mut buf = BytesMut::with_capacity(1024);
        let mut indent = "".to_string();
        if !self.pubs.has_samples() && !self.subs.has_samples() {
            return "No publisher and subscribers".into();
        }
        if self.pubs.has_samples() && self.subs.has_samples() {
            let _ = write!(
                buf,
                "{} pub/sub stats: {}\n",
                &self.name, &self.bench_sample
            );
            indent += "  ";
        }
        if self.pubs.has_samples() {
            let _ = write!(buf, "{}Pub stats: {}\n", indent, &self.pubs.group_sample);
            if self.pubs.samples.len() > 1 {
                for (i, stat) in self.pubs.samples.iter().enumerate() {
                    let _ = write!(
                        buf,
                        "{} [{}] {} ({} msgs)\n",
                        indent,
                        i + 1,
                        stat,
                        stat.job_msg_cnt,
                    );
                }
            }
        }
        if self.subs.has_samples() {
            let _ = write!(buf, "{}Sub stats: {}\n", indent, self.subs.group_sample);
            if self.subs.samples.len() > 1 {
                for (i, stat) in self.subs.samples.iter().enumerate() {
                    let _ = write!(
                        buf,
                        "{} [{}] {} ({} msgs)\n",
                        indent,
                        i + 1,
                        stat,
                        stat.job_msg_cnt,
                    );
                }
            }
        }
        let _ = write!(buf, "{} {}\n", indent, self.subs.statistics());
        String::from_utf8(buf.to_vec()).unwrap()
    }
    pub fn csv(&self) -> String {
        let buf = BytesMut::with_capacity(1024);
        let mut wtr = csv::Writer::from_writer(buf.writer());
        let headers = vec![
            "#RunID",
            "ClientID",
            "MsgCount",
            "MsgBytes",
            "MsgsPerSec",
            "BytesPerSec",
            "DurationSecs",
        ];
        wtr.write_record(&headers).unwrap();
        let mut pre = "S";
        let groups = vec![&self.subs, &self.pubs];
        for (i, g) in groups.iter().enumerate() {
            if i == 1 {
                pre = "P";
            }
            for (j, s) in g.samples.iter().enumerate() {
                let r = vec![
                    "runid".into(),
                    format!("{}{}", pre, j),
                    format!("{}", s.msg_count),
                    format!("{}", s.msg_bytes),
                    format!("{}", s.rate()),
                    format!("{}", s.throughput()),
                    format!("{}", s.duration().as_secs_f64()),
                ];
                wtr.write_record(r).unwrap();
            }
        }
        wtr.flush().unwrap();
        let buf = wtr.into_inner().unwrap().into_inner();
        String::from_utf8(buf.to_vec()).unwrap()
    }
}
// MsgsPerClient divides the number of messages by the number of clients and tries to distribute them as evenly as possible
pub fn msgs_per_client(num_msgs: usize, num_clients: usize) -> Vec<usize> {
    let mut counts = vec![0; num_clients];
    if num_clients == 0 || num_msgs == 0 {
        return counts;
    }
    let mc = num_msgs / num_clients;
    for i in 0..num_clients {
        counts[i] = mc;
    }
    let extra = num_msgs % num_clients;
    for i in 0..extra {
        counts[i] += 1;
    }
    counts
}
#[cfg(test)]
mod tests {
    use super::*;
    use std::ops::Add;

    const MSG_SIZE: usize = 8;
    const MILLION: usize = 1000 * 1000;
    use lazy_static::lazy_static;
    lazy_static! {
        static ref BASE_TIME: Instant = { Instant::now() };
    }
    fn million_messags_second_sample(seconds: i32) -> Sample {
        let messages = MILLION * seconds as usize;
        let start = BASE_TIME.clone();
        let end = start.add(Duration::from_secs(seconds as u64));
        let mut s = Sample::new(messages, MSG_SIZE, messages as u64, 0, start, end);
        s.msg_bytes = (messages * MSG_SIZE) as u64;
        s.io_bytes = s.msg_bytes;
        s
    }
    #[test]
    fn test() {}
    #[test]
    fn test_std_dev() {
        let mut sg = SampleGroup::default();
        sg.add_sample(million_messags_second_sample(1));
        sg.add_sample(million_messags_second_sample(1));
        sg.add_sample(million_messags_second_sample(1));
        assert_eq!(sg.std_dev(), 0.0);
    }
    #[test]
    fn test_bench_setup() {
        let mut bench = Benchmark::new("test");
        bench.add_pub_sample(million_messags_second_sample(1));
        bench.add_sub_sample(million_messags_second_sample(1));
        assert_eq!(bench.pubs.samples.len(), 1);
        assert_eq!(bench.subs.samples.len(), 1);
        assert_eq!(bench.bench_sample.msg_count as usize, 2 * MILLION);
        assert_eq!(bench.bench_sample.io_bytes as usize, 2 * MILLION * MSG_SIZE);
        assert_eq!(bench.bench_sample.duration(), Duration::from_secs(1));
    }
    fn make_bench(subs: usize, pubs: usize) -> Benchmark {
        let mut bench = Benchmark::default();
        for _ in 0..subs {
            bench.add_sub_sample(million_messags_second_sample(1));
        }
        for _ in 0..pubs {
            bench.add_pub_sample(million_messags_second_sample(1));
        }
        bench
    }
    #[test]
    fn test_csv() {
        let bench = make_bench(2, 3);
        let csv = bench.csv();
        println!("csv\n{}", csv);
    }
    #[test]
    fn test_report() {
        let bench = make_bench(2, 3);
        let r = bench.report();
        println!("r=\n{}", r);
    }
}
