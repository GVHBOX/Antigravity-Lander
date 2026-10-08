// Antigravity 2.0 Native Menu & Context Menu Localization Hook

//__MENU_DATA_PLACEHOLDER__

function translateMenu(menuItem) {
  if (!menuItem) return;
  if (menuItem.label && menuTranslationMap[menuItem.label]) {
    menuItem.label = menuTranslationMap[menuItem.label];
  }
  if (menuItem.submenu) {
    if (Array.isArray(menuItem.submenu.items)) {
      menuItem.submenu.items.forEach(translateMenu);
    } else if (Array.isArray(menuItem.submenu)) {
      menuItem.submenu.forEach(translateMenu);
    }
  }
}

//__CONTEXT_MENU_DATA_PLACEHOLDER__

function translateContextLabel(lbl) {
  if (!lbl) return '';
  return contextMenuTranslationMap[lbl] || lbl;
}

// 只挂载新增符号：整体替换 module.exports 会清掉宿主模块自己的导出，
// 宿主 main.js 随后调用 menu_1.setupApplicationMenu 就会抛错。
if (typeof module !== 'undefined' && module.exports) {
  module.exports.translateMenu = translateMenu;
  module.exports.translateContextLabel = translateContextLabel;
}
