# File Cacher

File Cacher is a CLI application that allows you to retrieve static content from the web and cache it in a local directory for faster access.

## Installing

To install File Cacher, just run build.sh and then install.sh

## Example usage

download a file and store it in \<cache-dir\>/readme.md
```sh
file-cacher get --output readme.md https://raw.githubusercontent.com/cr3eperall04/file-cacher/master/README.md
```

## Config file

there is a config file in $HOME/.config/file-cacher/config.conf

### Values:
- cache-db : the path for the json file containing information on the cached files
- cache-dir : where the file will be downloaded to
- file-default-lifetime : the average time in seconds for when the files will be re-downloaded
- random-offset-range : the random value range in seconds to add to file-default-lifetime



