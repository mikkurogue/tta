# TTA - TypeScript Type Analisys

this tool has been created to analyse types from typescript projects to find duplicate entries.

For now we have hardcoded to ignore node_modules and the .nx folder (as the project it is mainly used in is a nx monorepo)

## Usage

`tta` scans the current working directory recursively

`tta <dir>` scans the directory given in the cli

`tta --verbose` logs any errors that was found during analysis
