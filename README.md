# IDO Tool
A simple CLI tool to decompile .ido files, and compile raw file back to .ido file.

### Help
```
A TUI tool to compile and decompile .ido files.

Usage: idotool.exe --file <FILE> --output <OUTPUT> <--decompile|--compile>

Options:
  -d, --decompile
          Decompile .ido file

  -c, --compile
          Compile .xml file to .ido

  -f, --file <FILE>
          Input .ido file

  -o, --output <OUTPUT>
          Output file path

  -h, --help
          Print help (see a summary with '-h')

  -V, --version
          Print version
```

### Usage
```
> # Decompile .ido file
> idotool --decompile --file myidofile.ido --output rawidocontent

> # Compile raw IDO content to .ido
> idotool --compile --file myrawidocontent --output myidofile.ido
```