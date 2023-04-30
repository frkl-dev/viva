# viva

Note: ***very very alpha***

A generic environment wrapper and (in the future) script interpreter.

This project is in very early alpha state, and its main purpose is to serve as a library for another project I'm working on. That being said, I think the pattern I want it to encapsulate can be useful for other/similar circumstances, which is why it's published as a separate project.

In general, this is just a thin wrapper around the brilliant ['rattler' library](https://github.com/mamba-org/rattler), and most of the interesting stuff happens there. So, in most cases I'd recommond to use that library directly.

## Current (known) issues

- no versioning support for packages
- applications are not run within an activated environment, so if a (conda) package depends on (for example) populated environment variables (like is done when activating such an environment), then it will fail
- updating environments with new specs sometimes leaves environments in an unusable state

## Usage

### Examples

#### Create / ensure environments exist
```bash
# install the 'cookiecutter' package into the 'default' environment (if not already there)
viva -c conda-forge -s cookiecutter apply
```
#### Run commands in environments

```bash
# install the 'cookiecutter' package into an environment called 'project_templates' (if not already there), then run it
# note the '--' to separate the viva arguments from the command arguments
viva -e project_templates -c conda-forge -s cookiecutter run -- cookiecutter --help
```

#### List available environments

```bash
# list all existing environments
viva list-envs
```

#### Delete environments

```bash
# delete the environment called 'project-templates'
viva -e project_templates remove
```

## Environments

Each environment lives in a so-called 'target-prefix', where all (well, most, if I understand right) files are hard-linked into, which means that if you create 2 or more environments with the same packages, the space used would be equal to a single one (plus some small fileystem metadata). 

In addition, each environment has a spec (json) file that records which channels where used to create it, and also which packages (matchspecs).

The location of the environment and spec file depends on the OS, and how the environment was specified (see below). 

In case the environment was specified as a simple string representing an alias:

- the environment lives under the (OS-dependent) user data dir (use `viva list-envs` to see the actual path), plus `viva/envs/` 
- the env spec file lives under the (OS-dependent) user config dir, plus `viva/envs/`

Otherwise:

- target-prefix and environment spec files were specified in the command-line arguments, and will be created there

For now, I'm only interested in the former case, and the latter is only stubbed out, but I did not want to close the door to having this flexibility.

### Environment specification in the `-e` / `--env` command-line argument

In *viva*, an environment can be specified in several different ways by the env-spec string. *viva* parses the string in the following order, the first match will determine where the environment lives:

- if the string contains a (OS-specific) path separator (like `/` on Posix, '\\' on Windows):
  - the string will be interpreted as relative or absolute path:
    - if that path points to an existing file, that file will be read and its content will interpreted as spec data for the environment
    - if that path points to an existing directory, or does not exist, the environment will be created in that directory
- if no path separator is found, and the string does not contain any characters except for alphanumeric characters and `_`:
  - the string will be interpreted as the environment alias, and the environment will be created under this alias in the (also OS-specific) user data directory

## Featrues (current & planned)

- [X] Create environments
- [X] Merge environments
- [X] Delete environments
- [X] List environments
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
