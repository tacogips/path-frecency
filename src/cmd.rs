use clap::Parser;

#[derive(Parser, Debug)]
pub struct Add {
    pub path: String,
}

#[derive(Parser, Debug)]
pub struct Fetch {
    #[clap(short, long)]
    pub asc: bool,
    #[clap(short, long)]
    pub limit: Option<usize>,
    #[clap(short, long)]
    pub with_score: bool,
    #[clap(long)]
    pub sort_by_last_visit: bool,
}

#[derive(Parser, Debug)]
pub struct RemoveNotExists;
