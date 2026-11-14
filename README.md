## Path Frecensy

The frecensy algorithm based on https://wiki.mozilla.org/User:Jesse/NewFrecency


### Install
```
cargo install --git https://github.com/tacogips/path-frecency
```

### Usage

```
path-frecency 0.1.0
tacogips

USAGE:
    path-frecency [OPTIONS] <SUBCOMMAND>

OPTIONS:
    -d, --db-file <DB_FILE>
    -h, --help                 Print help information
    -V, --version              Print version information

SUBCOMMANDS:
    add                  Add path
    fetch                Show paths list orderd by frequency
    help                 Print this message or the help of the given subcommand(s)
    remove-not-exists    Remove paths that not exists anymore.
```
### Integrate with FZF and Zsh

Save the path when you `cd`. Type `fd` and you'll be able to navigate over the list of paths sorted by frecensy.

```zsh
# in ~/.zshrc_function

function chpwd_record_history() {
    echo $PWD | xargs path-frecency add
}

chpwd_functions=($chpwd_functions chpwd_record_history)

function fd(){
    dest=$(path-frecency fetch |  fzf +m --query "$LBUFFER" --prompt="cd > ")
		cd "$dest"
}

function fdclean(){
	path-frecency remove-not-exists
}


```
