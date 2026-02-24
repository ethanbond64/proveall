# ProveAll

ProveAll is a desktop code review tool that helps software developers keep track of their branches as they use AI to assist in writing code. With ProveAll, developers can review individual commits line by line in a full editor, raising issues when code needs to be fixed, storing notes on code that needs to be revisited, or approving code that has been verified, understood, and is ready to ship. By tracking reviews alongside the version control system, developers are able to get a full view of the state of their branch across multiple commit reviews.

The suggested workflow is to commit after each AI prompt completes, then switch over to ProveAll to review what was written. While reviewing you can optionally execute the next prompt which can be committed or discarded depending on the outcome of the previous commit's review. Once all commits have all been reviewed, developers can view a composite view of all changes alongside the list of open issues to address or resolve.

## Setup and Run

Note: Git is currently the only supported VCS.

To build and run this from source, you will need [Node.js](https://nodejs.org/) and [rustup](https://rustup.rs/).

Clone the repository, run `npm install` to install dependencies, then run `cargo tauri build` to build the Tauri application. The built application can be found in: `tauri_src/target/release/bundle/`
