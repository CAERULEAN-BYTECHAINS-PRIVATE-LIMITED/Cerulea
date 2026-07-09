const fs = require('fs');
const path = require('path');

// Folders to ignore
const IGNORE_DIRS = ['node_modules', '.git', '.next', '.vscode', 'dist', 'build', 'out', 'coverage'];

function getStructure(dir, prefix = '') {
  let output = '';
  let items;

  try {
    items = fs.readdirSync(dir, { withFileTypes: true });
  } catch (err) {
    return '';
  }

  // Sort: Folders first, then files
  items.sort((a, b) => {
    if (a.isDirectory() && !b.isDirectory()) return -1;
    if (!a.isDirectory() && b.isDirectory()) return 1;
    return a.name.localeCompare(b.name);
  });

  // Filter out ignored items
  const filteredItems = items.filter(item => !IGNORE_DIRS.includes(item.name));

  filteredItems.forEach((item, index) => {
    const isLast = index === filteredItems.length - 1;
    const marker = isLast ? '└── ' : '├── ';
    const newPrefix = prefix + (isLast ? '    ' : '│   ');

    output += `${prefix}${marker}${item.name}\n`;

    if (item.isDirectory()) {
      output += getStructure(path.join(dir, item.name), newPrefix);
    }
  });

  return output;
}

console.log('Generating project map...');
const map = getStructure(__dirname);
fs.writeFileSync('project_structure.txt', map);
console.log('Done! detailed list saved to: project_structure.txt');