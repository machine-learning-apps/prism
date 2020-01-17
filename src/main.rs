use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use std::thread;
use crossbeam::channel as channel;

use hyper::service::{make_service_fn, service_fn};
use hyper::{Body, Error, Request, Response, Server};
use crossbeam::queue::{SegQueue};


struct ReqChannel {
    request: Request<Body>,
    sender: channel::Sender<Response<Body>>
}


fn send_batch(q: Arc<SegQueue<ReqChannel>>, last_send: Arc<AtomicUsize>, batch_size: &usize) {
    // sending batch, reset timer
    let current_ts = SystemTime::now().duration_since(UNIX_EPOCH).unwrap();
    last_send.store(current_ts.as_millis() as usize, Ordering::Relaxed);
    for _ in 0..*batch_size {
        let r = q.pop();
        match r {
            Err(_e) => return,  // TODO: handle this. This can happen if another send consumed request while this func was running
            Ok(r) => r.sender.send(Response::new(Body::from(format!("Sending batch {:?}", current_ts.as_millis())))).unwrap()
        }
    };
}

fn send_timeout(q: Arc<SegQueue<ReqChannel>>, last_send: Arc<AtomicUsize>, timeout: &usize) {
    let mut next_send: usize = 0;
    loop {
        let ls = last_send.load(Ordering::Relaxed);
        let now = (SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_millis() as usize);
        if next_send <= now {
            if q.len() > 0 {
                loop {
                    let r = q.pop();
                    match r {
                        Err(_e) => break,
                        Ok(r) => r.sender.send(Response::new(Body::from(format!("Sending timeout {:?}", now)))).unwrap()
                    }
                }
            }
            last_send.store(now, Ordering::Relaxed);
            next_send = now + timeout;
        }
        else {
            next_send = ls + timeout;
        }
        thread::sleep_ms((next_send - now) as u32);
    }
}

#[tokio::main]
async fn main() {
    let batch_size = 3;  // TODO: make this cli option, max batch size
    let timeout: usize = 25000; // TODO: ditto, timeout in millisecond
    let addr = ([0, 0, 0, 0], 3000).into();  // TODO: configurable

    let current_ts = SystemTime::now().duration_since(UNIX_EPOCH).unwrap();

    let mut last_send = Arc::new(AtomicUsize::new(current_ts.as_millis() as usize));

    let q = Arc::new(SegQueue::new());  // q is our request queue

    // spawn 
    let qt = q.clone();
    let last_send_timeout = last_send.clone();
    thread::spawn(move || {
        send_timeout(qt, last_send_timeout, &timeout);
    });
    
    let make_service = make_service_fn(move |_| {
        let qq = q.clone();
        let last_send_queue = last_send.clone();
        async move {
            Ok::<_, Error>(service_fn(move |req| {
                let qqq = qq.clone();
                let last_send_queue = last_send_queue.clone();
                async move {
                    let (sender, receiver) = channel::unbounded();
                    let rc = ReqChannel{
                            request: req,
                            sender: sender
                        };
                    qqq.push(rc);
                    thread::spawn(move || {
                        if qqq.len() >= batch_size {
                            send_batch(qqq, last_send_queue, &batch_size);  // we've reached max batchsize, send to backend
                        }
                    });
                    
                    let resp = receiver.recv();
                    match resp {
                        Ok(r) => Ok::<_, Error>(r),
                        Err(e) => Ok::<_, Error>(Response::new(Body::from(format!("Error"))))
                    }
                    
                }
            }))
        }
    });

    let server = Server::bind(&addr).serve(make_service);

    println!("Listening on http://{}", addr);

    if let Err(e) = server.await {
        eprintln!("server error: {}", e);
    }
}