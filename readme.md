In this post, we'll define a Kubernetes Custom Resource Definition (CRD) and then write a controller (or operator) to manage it — all in 60 lines of Rust code.

Over the last several months, I have been writing more and more Kubernetes-specific code in Rust. Even though Kubernetes itself was written in Go, I am finding that I can typically write more concise, readable, and stable Kubernetes code in Rust. For example, I recently wrote functionally equivalent CRD controllers in Rust and in Go. The Go version was over 1,700 lines long and was loaded with boilerplate and auto-generated code. The Rust version was only 127 lines long. It was much easier to understand and debug...and definitely faster to write. Here, we'll write one in just 60 lines.

Getting Started
You should have the latest stable Rust release. You'll also need kubectl, configured to point to an existing Kubernetes cluster.

A controller runs as a daemon process, typically inside of a Kubernetes cluster. So we'll create a new Rust program (as opposed to a library). Our aim here is to provide a basic model for writing controllers, so we won't spend time breaking things down into modules. We also won't cover things like building a Rust Docker image or creating a Deployment to run our controller. All of that is well documented elsewhere.

Let's start by creating our new project:

1
$ cargo new k8s-controller
2
     Created binary (application) `k8s-controller` package
Before we start writing code, let's create two YAML files. The first is our CRD definition, and the second is an instance of that CRD. We'll create a directory in k8s-controller/ called docs/ and put our YAML files there.

The Custom Resource Definition looks like this:

1
apiVersion: apiextensions.k8s.io/v1beta1
2
kind: CustomResourceDefinition
3
metadata:
4
  name: books.example.technosophos.com
5
spec:
6
  group: example.technosophos.com
7
  versions:
8
    - name: v1
9
      served: true
10
      storage: true
11
  scope: Namespaced
12
  names:
13
    plural: books
14
    singular: book
15
    kind: Book


Stepping through this file is beyond the scope of this tutorial, but you can learn all about this file format in the official docs. (Recent versions of Kubernetes added more fields to the definition, but we're going to stick with a basic version.) A CRD is just a manifest that declares a new resource type and expresses the names that are associated with this new resource type. The full name of ours is books.example.technosophos.com/v1.

Next, let's make an instance of our Book CRD.

1
apiVersion: example.technosophos.com/v1
2
kind: Book
3
metadata:
4
  name: moby-dick
5
spec:
6
  title: Moby Dick
7
  authors:
8
    - Herman Melville


As with most Kubernetes resource types, our example above has two main sections:

metadata, which is a predefined metadata section
spec, which holds our custom body
We can quickly test to make sure that things are working:

1
$ kubectl create -f docs/crd.yaml
2
customresourcedefinition.apiextensions.k8s.io "books.example.technosophos.com" created
3
$ kubectl create -f docs/book.yaml
4
book.example.technosophos.com "moby-dick" created
5
$ kubectl delete book moby-dick
6
book.example.technosophos.com "moby-dick" deleted
We now have everything we need to start coding our new controller.

Setting Up Our Cargo.toml File
Rather than incrementally adding dependencies to our Cargo.toml file as we go, we'll just set up all of the dependencies now. As the text progresses, we'll see how these are used.

1
[package]
2
name = "k8s-controller"
3
version = "0.1.0"
4
edition = "2018"
5
​
6
[dependencies]
7
kube = "0.14.0"
8
serde = "1.0"
9
serde_derive = "1.0"
10
serde_json = "1.0"


The serde serialization libraries are likely already familiar to you. And kube is the Kubernetes library for writing controllers. (Another library, k8s_openapi, is useful for working with existing Kubernetes resource types, but we don't need it here)

Part 1: Create the Book Struct
The first piece of code we'll write is a struct that represents our book CRD. And the easiest way to start with that is to write the basic struct that defines the body ( spec). In our book.yaml we had two fields in spec:

Since we're just writing a quick example, we'll go ahead and create this struct inside of main.rs:

1
#[macro_use]
2
extern crate serde_derive;
3
​
4
// This is our new Book struct
5
#[derive(Serialize, Deserialize, Clone, Debug)]
6
pub struct Book {
7
    pub title: String,
8
    pub authors: Option<Vec<String>>,
9
}
10
​
11
// This was the boilerplate that Cargo generated:
12
fn main() {
13
    println!("Hello, world!");
14
}


By making the title a string and the authors an Option, we're stating that the title is required, but the authors are not. So now we have:

A title string
An optional vector of authors as strings
We've also used macros to generate the Serde serializer and deserializer features as well as clone and debug support.

If we look again at our book.yaml, we will see that the body of the book has two sections:

metadata with the name
spec with the rest of the data
Some Kubernetes objects have a third section called status. We don't need one of those.

The kube library is aware of this metadata/ spec/ status pattern. So it provides a generic type called kube::api::Object that we can use to create a Kubernetes-style resource. To make our code easier to read, we'll create a type alias for this new resource type:

1
// Describes a Kubernetes object with a Book spec and no status
2
type KubeBook = Object<Book, Void>;


A cube::api::Object already has the metadata section defined. But it gives us the option of adding our own spec and status fields. We add Book as the spec, but we don't need a status field, so we set it to Void.

Here's the code so far:

1
#[macro_use]
2
extern crate serde_derive;
3
​
4
use kube::api::{Object, Void};
5
​
6
#[derive(Serialize, Deserialize, Clone, Debug)]
7
pub struct Book {
8
    pub title: String,
9
    pub authors: Option<Vec<String>>,
10
}
11
​
12
// This is a convenience alias that describes the object we get from Kubernetes
13
type KubeBook = Object<Book, Void>;
14
​
15
fn main() {
16
    println!("Hello, world!");
17
}


Now we're ready to work on main().

Part 2: Connecting to Kubernetes
Next, we'll create the controller in the main() function. We'll take this in a few steps. First, let's load all of the information we need in order to work with Kubernetes.

1
#[macro_use]
2
extern crate serde_derive;
3
​
4
use kube::{
5
    api::{Object, Void, RawApi},
6
    client::APIClient,
7
    config,
8
};
9
​
10
​
11
#[derive(Serialize, Deserialize, Clone, Debug)]
12
pub struct Book {
13
    pub title: String,
14
    pub authors: Option<Vec<String>>,
15
}
16
​
17
// This is a convenience alias that describes the object we get from Kubernetes
18
type KubeBook = Object<Book, Void>;
19
​
20
fn main() {
21
    // Load the kubeconfig file.
22
    let kubeconfig = config::load_kube_config().expect("kubeconfig failed to load");
23
​
24
    // Create a new client
25
    let client = APIClient::new(kubeconfig);
26
​
27
    // Set a namespace. We're just hard-coding for now.
28
    let namespace = "default";
29
​
30
    // Describe the CRD we're working with.
31
    // This is basically the fields from our CRD definition.
32
    let resource = RawApi::customResource("books")
33
        .group("example.technosophos.com")
34
        .within(&namespace);
35
​
36
}


If we run this program it won't do anything visible. But here's what's happening in the main() function:

First we load the kubeconfig file (or, in cluster, read the secrets out of the volume mounts). This loads the URL to the Kubernetes API server, and also the credentials for authenticating.
Second, we create a new API client. This is the object that will communicate with the Kubernetes API server.
Third, we set the namespace. Kubernetes segments objects by namespaces. In a normal program, we'd provide a way for the user to specify a particular namespace. But for this, we'll just use the default built-in namespace.
Forth, we are creating a resource that describes our CRD. We'll use this in a bit to tell the informer which things it should watch for.
So now we have sufficient information to run operations against the Kubernetes API server for our particular namespace and watch for our particular CRD.

Next, we can create an informer.

Part 3: Creating an Informer
In Kubernetes parlance, an informer is a special kind of agent that watches the Kubernetes event stream and informs the program when a particular kind of resource triggers an event. This is the heart of our controller.

There is a second kind of watching agent that keeps a local cache of all objects that match a type. That is called a reflector.

In our case, we're going to write an informer that tells us any time anything happens to a Book.

Here's the code to create an informer and then handle events as they come in:

1
#[macro_use]
2
extern crate serde_derive;
3
​
4
use kube::{
5
    api::{Object, RawApi, Informer, WatchEvent, Void},
6
    client::APIClient,
7
    config,
8
};
9
​
10
#[derive(Serialize, Deserialize, Clone, Debug)]
11
pub struct Book {
12
    pub title: String,
13
    pub authors: Option<Vec<String>>,
14
}
15
​
16
// This is a convenience alias that describes the object we get from Kubernetes
17
type KubeBook = Object<Book, Void>;
18
​
19
fn main() {
20
    // Load the kubeconfig file.
21
    let kubeconfig = config::load_kube_config().expect("kubeconfig failed to load");
22
​
23
    // Create a new client
24
    let client = APIClient::new(kubeconfig);
25
​
26
    // Set a namespace. We're just hard-coding for now.
27
    let namespace = "default";
28
​
29
    // Describe the CRD we're working with.
30
    // This is basically the fields from our CRD definition.
31
    let resource = RawApi::customResource("books")
32
        .group("example.technosophos.com")
33
        .within(&namespace);
34
​
35
    // Create our informer and start listening.
36
    let informer = Informer::raw(client, resource).init().expect("informer init failed");
37
    loop {
38
        informer.poll().expect("informer poll failed");
39
​
40
        // Now we just do something each time a new book event is triggered.
41
        while let Some(event) = informer.pop() {
42
            handle(event);
43
        }
44
    }
45
}
46
​
47
fn handle(event: WatchEvent<KubeBook>) {
48
    println!("Something happened to a book")
49
}


In the code above, we've added a new informer:

1
let informer = Informer::raw(client, resource).init().expect("informer init failed");


This line creates a raw informer. A raw informer is one that does not use the Kubernetes OpenAPI spec to decode its contents. Since we are using a custom CRD, we don't need the OpenAPI spec. Note that we give this informer two pieces of information:

A Kubernetes client that can talk to the API server
The resource that tells the informer what we want to watch for
Based on these pieces of information, our informer will now connect to the API server and watch for any events having to do with our Book CRD. Next, we just need to tell it to keep listening for new events:

1
 loop {
2
    informer.poll().expect("informer poll failed");
3
​
4
    // Now we just do something each time a new book event is triggered.
5
    while let Some(event) = informer.pop() {
6
        handle(event);
7
    }
8
}


The above tells the informer to poll the API server. Each time a new event is queued, pop() takes the event off of the queue and handles it. Right now, our handle() method is unimpressive:

1
fn handle(event: WatchEvent<KubeBook>) {
2
    println!("Something happened to a book")
3
}


In a moment, we'll add some features to handle(), but first let's see what happens if we run this code.

In one terminal, start cargo run and leave it running.

1
$ cargo run
2
    Finished dev [unoptimized + debuginfo] target(s) in 7.28s
3
     Running `target/debug/k8s-controller`
Make sure your local environment is pointed to a Kubernetes cluster! Otherwise neither cargo run nor the kubectl commands will work. And make sure you installed docs/crd.yaml.

Now, with that running in one terminal, we can run this in another:

1
$ kubectl create -f docs/book.yaml
2
# wait for a bit
3
$ kubectl delete book moby-dick


In the cargo run console, we'll see this:

1
    Finished dev [unoptimized + debuginfo] target(s) in 7.28s
2
     Running `target/debug/k8s-controller`
3
Something happened to a book
4
Something happened to a book
In the final section, we'll add a little more to the handle() function.

Part 4: Handling Events
In this last part, we'll add a few more things to the handle() function. Here is our revised function:

1
fn handle(event: WatchEvent<KubeBook>) {
2
    // This will receive events each time something 
3
    match event {
4
        WatchEvent::Added(book) => {
5
            println!("Added a book {} with title '{}'", book.metadata.name, book.spec.title)
6
        },
7
        WatchEvent::Deleted(book) => {
8
            println!("Deleted a book {}", book.metadata.name)
9
        }
10
        _ => {
11
            println!("another event")
12
        }
13
    }
14
}


Note that the function signature says that it accepts event: WatchEvent<KubeBook>. The informer emits WatchEvent objects that describe the event that it saw occur on the Kubernetes event stream. When we created the informer, we told it to watch for a resource that described our Book CRD.

So each time a WatchEvent is emitted, it will wrap a KubeBook object. And that object will represent our earlier YAML definition:

1
apiVersion: example.technosophos.com/v1
2
kind: Book
3
metadata:
4
  name: moby-dick
5
spec:
6
  title: Moby Dick
7
  authors:
8
    - Herman Melville


So we would expect that a KubeBook would have fields like book.metadata.name or book.spec.title. In fact, all of the attributes of our earlier Book struct will be available on the book.spec.

There are four possible WatchEvent events:

WatchEvent::Added: A new book CRD instance was created
WatchEvent::Deleted: An existing book instance was deleted
WatchEvent::Modified: An existing book instance was changed
WatchEvent::Error: An error having to do with the book watcher occurred
In our code above, we use a match event to match on one of the events. We explicitly handle Added and Deleted, but capture the others with the generic _ match.

To look closer, in the first match we simply print out the book object's name and the book's title:

1
WatchEvent::Added(book) => {
2
    println!("Added a book {} with title '{}'", book.metadata.name, book.spec.title)
3
},


If we execute cargo run and then run our kubectl create and kubectl delete commands again, this is what we'll see in the cargo run output:

1
$ cargo run
2
   Compiling k8s-controller v0.1.0 (/Users/technosophos/Code/Rust/k8s-controller)
3
    Finished dev [unoptimized + debuginfo] target(s) in 5.33s
4
     Running `target/debug/k8s-controller`
5
Added a book moby-dick with title 'Moby Dick'
6
Deleted a book moby-dick
From here, we might want to do something more sophisticated with our informer. Or we might want to instead experiment with a reflector. But in just 60 lines of code we have written an entire Kubernetes controller with a Custom Resource Definition!

Conclusion
That is all there is to creating a basic controller. Unlike writing these in Go, you won't need special code generators or annotations, gobs of boilerplate code, and complex configurations. This is a fast and efficient way of creating new Kubernetes controllers.

From here, you may want to take a closer look at the kube library's documentation. There are dozens of examples, and the API itself is well documented. You will also learn how to work with built-in Kubernetes types (also an easy thing to do).

The code for this post is available at github.com/technosophos/rust-k8s-controller.

You can find the final code in the GitHub copy of main.rs.