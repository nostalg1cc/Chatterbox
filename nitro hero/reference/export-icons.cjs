const React = require('react');
const {renderToStaticMarkup} = require('react-dom/server');
const fs = require('fs');
for (const name of ['ChevronDown','ChevronLeft','ChevronRight','CodeFork','Ellipsis','Picture','Pin','QrCode','Star','TextAlignCenter','TextAlignJustify','TextAlignLeft','TextAlignRight','ThumbsDown','ThumbsUp','Video']) {
  const Icon = require('@gravity-ui/icons/' + name).default;
  fs.writeFileSync('../assets/icons/' + name + '.svg', renderToStaticMarkup(React.createElement(Icon)));
}
