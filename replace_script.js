const fs = require('fs');
const path = require('path');

const walkSync = (dir, filelist = []) => {
  fs.readdirSync(dir).forEach(file => {
    const dirFile = path.join(dir, file);
    if (fs.statSync(dirFile).isDirectory()) {
      if (file !== 'node_modules' && file !== 'dist') {
        filelist = walkSync(dirFile, filelist);
      }
    } else if (dirFile.endsWith('.ts') || dirFile.endsWith('.tsx')) {
      filelist.push(dirFile);
    }
  });
  return filelist;
};

const files = walkSync('./web/src');
let changedFiles = 0;
for (const file of files) {
  let content = fs.readFileSync(file, 'utf8');
  let newContent = content.replace(/([a-zA-Z0-9_\]\)])\.(endsWith|startsWith|toLowerCase|includes)\(/g, '$1?.$2(');
  
  if (content !== newContent) {
    fs.writeFileSync(file, newContent, 'utf8');
    console.log('Updated', file);
    changedFiles++;
  }
}
console.log(`Finished. Updated ${changedFiles} files.`);
