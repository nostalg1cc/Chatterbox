const React = require('react');
const {renderToStaticMarkup} = require('react-dom/server');
const fs = require('fs');
for (const name of ['Bold','Italic','Underline','Strikethrough']) {
 const Icon = require('@gravity-ui/icons/' + name).default;
 fs.writeFileSync('../assets/icons/' + name + '.svg',renderToStaticMarkup(React.createElement(Icon)));
}
