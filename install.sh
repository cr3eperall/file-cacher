#!/usr/bin/bash
install -Dm755 -o root -g root ./target/file-cacher /usr/local/bin
install -Dm644 -o root -g root ./target/_file-cacher /usr/share/zsh/site-functions
install -Dm644 -o root -g root ./target/file-cacher.bash /usr/share/bash-completion/completions
