# viva

A generic environment wrapper and (in the future) script interpreter.

## Usage

### Examples

```bash
# install the 'cookiecutter' package into the 'default' environment (if not already there)
viva -c conda-forge -s cookiecutter apply
# install the 'cookiecutter' package into an environment called 'cc' (if not already there), then run it
viva -e cc -c conda-forge -s cookiecutter run -- cookiecutter --help
```

### Environment specification

In *viva*, an environment can be specified in several different ways by the env-spec string. *viva* parses the string in teh following order, the first match will determine where the environment lives:

- if the string contains a (OS-specific) path separator (like `/` on Posix, '\' on Windows):
  - the string will be interpreted as relative or absolute path:
    - if that path points to an existing file, that file will be read and its content will interpreted as spec data for the environment
    - if that path points to an existing directory, or does not exist, the environment will be created in that directory
- if no path separator is found, and the string does not contain any characters except for alphanumeric characters and `_`:
  - the string will be interpreted as the environment alias, and the environment will be created under this alias in the (also OS-specific) user data directory

## Featrues (current & planned)

- [X] Create environments
- [X] Merge environments
- [ ] Delete environments
- [ ] List environments
- [ ] Activate environments
- [ ] Fine-grained package specification (incl. versioning)
- [ ] `viva` script interpreter
- [ ] curly bash script template and generator

## Copyright & license

This project is MPL v2.0 & BSD-3 Clause licensed, for the license texts please check the [LICENSES](/LICENSES) file in this repository.

Code under `src/rattler`:
- Copyright (c) 2023 Bas Zalmstra
- License: BSD-3 Clause

Everything else:
- Copyright (c) 2023 Markus Binsteiner
- License: MPL v2.0
