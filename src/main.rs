#[macro_use]
extern crate serde_derive;

use kube::{
    api::{Object, Void, RawApi},
    client::APIClient,
    config,
};

//This is our new Book strucct
#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct Book {
    pub title: String,
    pub authors: Option<Vec<String>>,
}

//This is a convenience alias that describes the object we get from Kubenetes
type KubeBook = Object<Book, Void>;

//This was the boilerplate that cargo generated:
fn main() {
    // println!("Hello, world!");
    // Load the kubeconfig file.
    let kubeconfig = config::load_kube_config().expect("kubeconfig failed to load");
    
    // Create a new client
    let client = APIClient::new(kubeconfig);

    // Set a namespace. We're just hard-coding for now.
    let namespace = "default";

    // Describe the CRD we're working with.
    // This is basically the fields from our CRD definition.
    let resource = RawApi::customResource("books")
        .group("example.technosophos.com")
        .within(&namespace);
    
    // Create our informer and start listening.
    let informer = Informer::raw(client, resource).init().expect("informer init failed");
    loop {
        informer.poll().expect("informer poll failed");

        // Now we just do something each time a new book event is triggered.
        while let Some(event) = informer.pop() {
            handle(event);
        }
    }
}

fn handle(event: WatchEvent<KubeBook>) {
    // println!("Something happened to a book")
    // This will receive events each time something
    match event {
        WatchEvent::Added(book) => {
        println!("Added a book {} with title '{}'", book.metadata.name, book.spec.title)
        },
        WatchEvent::Deleted(book) => {
            println!("Deleted a book {}", book.metadata.name)
        },
        _ => {
            println!("another event")
        }
    }
}