use std::{
    sync::{mpsc, Arc, Mutex},
    thread,
};

pub struct Worker {
    id: usize,
    thread: Option<thread::JoinHandle<()>>,
}

pub type Job = Box<dyn FnOnce() + Send + 'static>;

pub struct ThreadPool {
    workers: Vec<Worker>,
    sender: Option<mpsc::Sender<Job>>,
}

impl Worker {
    pub fn new(id: usize, receiver: Arc<Mutex<mpsc::Receiver<Job>>>) -> Worker {
        // Crée un thread infini qui exécute un job à chaque tour de boucle.
        let thread = thread::spawn(move || loop {
            let message = receiver.lock().unwrap().recv();

            match message {
                Ok(job) => {
                    // TODO: Retirer ce print du code final.
                    println!("Worker {id} got a job. Executing it...");

                    job();
                }
                Err(err) => {
                    eprintln!("Worker {id} disconnected: {err}. Shutting down...");

                    break;
                }
            }
        });

        Worker {
            id,
            thread: Some(thread),
        }
    }
}

impl ThreadPool {
    pub fn build(size: usize) -> Result<ThreadPool, String> {
        assert!(size > 0, "ThreadPool need at least a size of 1");

        // Crée un vecteur avec une taille pré-allouée de 'size' éléments.
        let mut workers = Vec::with_capacity(size);

        // Crée un flux permettant d'écrire dans 'tx' pour recevoir dans 'rx'.
        // Utilisable à travers les threads. Cela permet de rendre le programme thread-safe.
        let (tx, rx) = mpsc::channel();

        // 'Arc' pour Atomic Reference Counter permet d'utiliser un compteur de références à
        // travers les threads pour rendre le programme thread-safe.
        let rx = Arc::new(Mutex::new(rx));

        for id in 0..size {
            workers.push(Worker::new(id, Arc::clone(&rx)));
        }

        Ok(ThreadPool {
            workers,
            sender: Some(tx),
        })
    }

    /// Execute a function or a closure in the next available thread in the pool.
    /// If no thread is available, it will be queued.
    pub fn execute<F>(&self, f: F)
    where
        F: FnOnce() + Send + 'static,
    {
        let job = Box::new(f);

        self.sender.as_ref().unwrap().send(job).unwrap();
    }
}

impl Drop for ThreadPool {
    fn drop(&mut self) {
        // Coupe le flux permettant de communiquer aux threads.
        // Cela arrêtera les threads en attentes de communication.
        drop(self.sender.take());

        // Attend que tous les threads aient fini leur exécution pour une fermeture propre de la
        // ThreadPool.
        for worker in &mut self.workers {
            // TODO: Retirer ce print du code final.
            println!("Shutting down worker {}.", worker.id);

            // La méthode 'join' a besoin d'un ownership du thread.
            // Une technique permettant de récupérer l'ownership d'une valeur qui appartient déjà à
            // un autre élément est de la mettre dans une 'Option' et de la récupérer grâce à la
            // méthode 'take'.
            if let Some(thread) = worker.thread.take() {
                // TODO: Gérer les erreurs générées par 'join'.
                thread.join().unwrap();
            }
        }
    }
}
