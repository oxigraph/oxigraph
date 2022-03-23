#! /usr/bin/env node

const fs = require('fs')

// We copy file to the new directory
fs.mkdirSync('pkg')
for (const file of fs.readdirSync('./pkg-web')) {
  fs.copyFileSync('./pkg-web/' + file, './pkg/' + file)
}
for (const file of fs.readdirSync('./pkg-node')) {
  fs.copyFileSync('./pkg-node/' + file, './pkg/' + file)
}

const pkg = JSON.parse(fs.readFileSync('./pkg/package.json'))
pkg.name = 'oxigraph'
pkg.main = 'node.js'
pkg.browser = 'web.js'
pkg.files = [
  '*.{js,wasm,d.ts}'
]
pkg.homepage = 'https://github.com/oxigraph/oxigraph/tree/main/js'
pkg.bugs = {
  url: 'https://github.com/oxigraph/oxigraph/issues'
}
pkg.collaborators = undefined
pkg.repository = {
  type: 'git',
  url: 'https://github.com/oxigraph/oxigraph.git',
  directory: 'js'
}
fs.writeFileSync('./pkg/package.json', JSON.stringify(pkg, null, 2))
