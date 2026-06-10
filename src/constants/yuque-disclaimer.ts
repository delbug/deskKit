/** 语雀导出模块免责声明（界面与文档共用文案） */

export const YUQUE_DISCLAIMER_TITLE = '使用须知与免责声明';

export const YUQUE_DISCLAIMER_SHORT =
  '语雀导出内容仅供个人学习研究，请遵守语雀服务协议；建议 24 小时内删除本地副本，勿传播或用于违法违规用途。';

export const YUQUE_DISCLAIMER_LINES = [
  '本功能仅供个人学习、研究和技术交流，请勿用于商业推广、批量爬取、侵权转载或其他违法违规用途。',
  '请仅导出您本人拥有合法访问权限的内容，并遵守语雀《服务协议》《隐私权政策》及著作权等相关法律法规。',
  '建议在完成学习或备份目的后，于 24 小时内自行删除本地导出文件，勿长期留存、传播或二次发布。',
  'DeskKit 为第三方开源工具，与语雀（阿里巴巴）无任何官方关联；因使用者不当操作产生的纠纷或后果，由使用者自行承担，开发者不承担法律责任。',
] as const;

export const YUQUE_DISCLAIMER_CONFIRM = [
  ...YUQUE_DISCLAIMER_LINES,
  '',
  '点击「我已阅读并同意」即表示您已理解并接受上述条款。',
].join('\n');
