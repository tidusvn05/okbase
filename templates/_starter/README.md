# Create your own template

Copy this directory into your own repository. Change `template.yaml`, replace the bundle content,
and check it without writing a destination:

```sh
okbase -b ./preview-kb init --template-path ./my-template --print
```

Publish the directory as the repository root for a single template. For a collection, add a root
`catalog.yaml` with `format_version: 1` and `templates: [{path: my-template}]`.
The [format specification](https://github.com/tidusvn05/okbase/blob/main/docs/template-format.md) describes the complete contract.
This starter is an authoring example and is not listed in the built-in catalog.
