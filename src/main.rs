use std::sync::Arc;

use std::thread;
use crossbeam::channel as channel;

use hyper::service::{make_service_fn, service_fn};
use hyper::{Body, Error, Request, Response, Server};
use crossbeam::queue::{SegQueue};


struct ReqChannel {
    request: Request<Body>,
    sender: channel::Sender<Response<Body>>
}


fn send_batch(q: Arc<SegQueue<ReqChannel>>, batch_size: usize) {
    for n in 0..batch_size {
        let r = q.pop();
        match r {
            Err(e) => return,
            Ok(r) => r.sender.send(Response::new(Body::from(format!("Sending batch")))).unwrap()
        }
    };
}

#[tokio::main]
async fn main() {
    let batch_size = 3;  // TODO: make this cli option, max batch size
    let timeout = 0.5; // TODO: ditto, timeout in seconds
    let addr = ([0, 0, 0, 0], 3000).into();  // TODO: configurable

    let q = Arc::new(SegQueue::new());  // q is our request queue
    
    let make_service = make_service_fn(move |_| {
        let qq = q.clone();
        async move {
            Ok::<_, Error>(service_fn(move |req| {
                let qqq = qq.clone();
                async move {
                    let (sender, receiver) = channel::unbounded();
                    let rc = ReqChannel{
                            request: req,
                            sender: sender
                        };
                    qqq.push(rc);
                    thread::spawn(move || {
                        if qqq.len() >= batch_size {
                            send_batch(qqq, batch_size);
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