// Sample data for the OpenDict prototype.
// Every dictionary, entry, entry count and file size below was written for this
// prototype. None of it comes from a real dictionary.

export const DICTS = [
  { id: 'ec', mark: '英', name: '样例英汉大词典', lang: 'zh', entries: 4120, size: '38 MB',
    css: `.e{font:14px/1.65 "Microsoft YaHei","PingFang SC",sans-serif;color:#222}.hw{font-size:18px;font-weight:700;color:#1a56b0}.ph{color:#777;margin-left:6px}.pos{font-style:italic;color:#a04a00;margin-left:6px}ol{margin:4px 0 4px 22px;padding:0}.ex{font-size:13px;color:#555}.der{font-size:13px;margin-top:4px}a{color:#1a56b0}` },
  { id: 'ld', mark: 'L', name: "Sample Learner's Dictionary", lang: 'en', entries: 3260, size: '112 MB',
    css: `.e{font:14.5px/1.6 Georgia,"Times New Roman",serif;color:#1d1d1d}.hw{font:700 19px Arial,Helvetica,sans-serif;color:#b3261e}.pos{color:#0a7a3d;font-style:italic}.gr{color:#666;font-size:13px}.df{margin:4px 0 0}.n{font:700 13px Arial,sans-serif;margin-right:6px}ul{margin:2px 0 6px 20px;padding:0;color:#234a8c;font-style:italic}` },
  { id: 'et', mark: '源', name: 'Sample Etymology', lang: 'en', entries: 980, size: '6 MB',
    css: `.e{font:15px/1.65 "Times New Roman",Times,serif;color:#222}.yr{font-weight:700}i{color:#5a3d1b}` },
  { id: 'ej', mark: '和', name: '見本英和辞典', lang: 'ja', entries: 2740, size: '54 MB',
    css: `.e{font:14.5px/1.75 "Yu Mincho","Hiragino Mincho ProN","MS Mincho",serif;color:#1a1a1a}.hw{font:700 17px "Yu Gothic","Hiragino Sans",sans-serif;color:#0b5c4d}.pos{font-size:12px;border:1px solid #0b5c4d;color:#0b5c4d;padding:0 3px;margin:0 6px}.x{display:block;color:#444}` },
  { id: 'th', mark: '同', name: '样例同义词典', lang: 'en', entries: 1510, size: '9 MB',
    css: `.e{font:14px/1.6 "Segoe UI",system-ui,sans-serif;color:#222}.hw{font-weight:700;font-size:17px}.sense{color:#00696e;font-weight:600;margin-top:6px}.syn a{color:#222;text-decoration:underline dotted;margin-right:12px}` },
  { id: 'kk', mark: '国', name: '見本国語辞典', lang: 'ja', entries: 5300, size: '71 MB',
    css: `.e{font:15px/1.8 "Yu Mincho","Hiragino Mincho ProN","MS Mincho",serif;color:#111}.hw{font-weight:700;font-size:18px}.pos{font-size:12px;margin:0 6px;color:#555}.s{display:block;padding-left:1.4em;text-indent:-1.4em}.x{color:#555}` },
  { id: 'jc', mark: '日', name: '样例日汉词典', lang: 'zh', entries: 2890, size: '27 MB',
    css: `.e{font:14px/1.7 "Microsoft YaHei","PingFang SC","Yu Gothic",sans-serif;color:#222}.hw{font-size:18px;font-weight:700;color:#7a1f5c}.rd{color:#7a1f5c;margin-left:6px}.ac{font-size:12px;border:1px solid #999;border-radius:8px;padding:0 5px;margin-left:4px}.pos{color:#666;margin-left:6px}ol{margin:4px 0 4px 22px;padding:0}.ex{font-size:13px;color:#555}` },
  { id: 'xh', mark: '汉', name: '样例现代汉语词典', lang: 'zh', entries: 6050, size: '19 MB',
    css: `.e{font:15px/1.8 "Songti SC","SimSun","Noto Serif SC",serif;color:#111}.hw{font-size:19px;font-weight:700}.py{font-family:"Times New Roman",serif;margin-left:8px;color:#8a1c1c}.x{font-family:"KaiTi","STKaiti",serif;color:#444}` },
];

// Groups are named by the user, so their names are not translated.
export const GROUPS = [
  { id: 'read', name: '精读', dicts: ['ec', 'ld', 'et', 'ej', 'th'] },
  { id: 'ja', name: '日本語', dicts: ['kk', 'jc', 'ej'] },
  { id: 'all', name: '全部', dicts: ['ec', 'ld', 'et', 'ej', 'th', 'kk', 'jc', 'xh'] },
];

const ec = (hw, ph, pos, body) => `<span class="hw">${hw}</span><span class="ph">${ph}</span><span class="pos">${pos}</span>${body}`;
const ld = (hw, pos, body) => `<span class="hw">${hw}</span> <span class="pos">${pos}</span>${body}`;
const ej = (hw, pos, body) => `<span class="hw">${hw}</span><span class="pos">${pos}</span>${body}`;

export const ENTRIES = {
  ec: {
    serendipity: ec('serendipity', '/ˌserənˈdɪpəti/', 'n.', ' [U]<ol><li>意外发现珍奇事物的本领；机缘凑巧<div class="ex">The discovery was pure serendipity. 这一发现纯属机缘巧合。</div></li><li>偶然发现的有趣或珍贵之物</li></ol><div class="der">派生：<a href="entry://serendipitous">serendipitous</a> <i>adj.</i></div>'),
    serendipitous: ec('serendipitous', '/ˌserənˈdɪpətəs/', 'adj.', '<ol><li>偶然发现的；机缘巧合的<div class="ex">a serendipitous meeting 一次巧遇</div></li></ol><div class="der">参见：<a href="entry://serendipity">serendipity</a></div>'),
    serene: ec('serene', '/səˈriːn/', 'adj.', '<ol><li>平静的；安详的<div class="ex">She looked serene after the long walk. 长途散步后她神情安详。</div></li><li>（天空、天气）晴朗的；无云的</li></ol><div class="der">派生：<a href="entry://serenity">serenity</a> <i>n.</i></div>'),
    serenity: ec('serenity', '/səˈrenəti/', 'n.', ' [U]<ol><li>平静；宁静；安详<div class="ex">the serenity of the lake at dawn 黎明时湖面的宁静</div></li></ol>'),
    serial: ec('serial', '/ˈsɪəriəl/', 'adj.', '<ol><li>连续的；顺序排列的</li><li>连载的；分期播出的</li></ol><span class="pos">n.</span><ol><li>连载小说；连续剧</li></ol>'),
    series: ec('series', '/ˈsɪəriːz/', 'n.', ' (pl. series)<ol><li>一系列；连续；接连<div class="ex">a series of experiments 一系列实验</div></li><li>系列节目；丛书</li></ol>'),
    serif: ec('serif', '/ˈserɪf/', 'n.', '<ol><li>衬线（字母笔画末端的装饰细线）<div class="ex">a serif typeface 衬线字体</div></li></ol>'),
    serious: ec('serious', '/ˈsɪəriəs/', 'adj.', '<ol><li>严重的；危急的<div class="ex">a serious mistake 严重的错误</div></li><li>认真的；严肃的；不开玩笑的</li><li>需要认真思考的；重要的</li></ol>'),
    ephemeral: ec('ephemeral', '/ɪˈfemərəl/', 'adj.', '<ol><li>短暂的；瞬息的<div class="ex">ephemeral pleasures 转瞬即逝的欢乐</div></li><li>（植物、昆虫）生命短促的</li></ol>'),
    run: ec('run', '/rʌn/', 'v.', ' (ran, run)<ol><li>跑；奔跑<div class="ex">He ran to catch the bus. 他跑着去赶公共汽车。</div></li><li>经营；管理<div class="ex">She runs a small bookshop. 她经营一家小书店。</div></li><li>（机器）运转；（程序）运行</li><li>（液体）流动；流淌</li><li>参加竞选</li><li>持续；延续<div class="ex">The play ran for two years. 这部戏连演了两年。</div></li></ol><span class="pos">n.</span><ol><li>跑步；奔跑</li><li>一段时期；一连串<div class="ex">a run of good luck 一连串好运</div></li><li>（演出的）连续上演期</li></ol>'),
  },
  ld: {
    serendipity: ld('serendipity', 'noun', ' <span class="gr">[uncountable]</span><p class="df">the fact of finding something interesting or valuable by chance, when you were not looking for it</p><ul><li>She came across the quotation by serendipity while searching for something else.</li><li>Many advances in science owe as much to serendipity as to planning.</li></ul>'),
    serendipitous: ld('serendipitous', 'adjective', '<p class="df">happening by chance in a lucky or useful way</p><ul><li>a serendipitous discovery in a second-hand bookshop</li></ul>'),
    serene: ld('serene', 'adjective', '<p class="df">calm and peaceful, with nothing to disturb you</p><ul><li>The lake lay still and serene in the morning light.</li></ul>'),
    serenity: ld('serenity', 'noun', ' <span class="gr">[uncountable]</span><p class="df">the quality of being calm and peaceful</p><ul><li>He envied the serenity of people who never check the time.</li></ul>'),
    serial: ld('serial', 'noun', '<p class="df">a story that is broadcast or published in several separate parts</p><span class="pos">adjective</span> <span class="gr">[only before noun]</span><p class="df">arranged in a series, or forming part of one</p><ul><li>Write the serial number on the form.</li></ul>'),
    series: ld('series', 'noun', ' <span class="gr">(plural series)</span><p class="df"><span class="n">1</span>several events or things of a similar kind that come one after another</p><ul><li>The lectures were followed by a series of questions.</li></ul><p class="df"><span class="n">2</span>a set of programmes or books that deal with the same subject or have the same characters</p>'),
    serif: ld('serif', 'noun', '<p class="df">a short line at the end of the main strokes of a printed letter</p><ul><li>The body text is set in a serif face.</li></ul>'),
    serious: ld('serious', 'adjective', '<p class="df"><span class="n">1</span>bad or dangerous enough to make you worried</p><ul><li>The storm caused serious damage to the roof.</li></ul><p class="df"><span class="n">2</span>thinking carefully about things and not joking</p><ul><li>Be serious for a moment. This matters.</li></ul>'),
    ephemeral: ld('ephemeral', 'adjective', ' <span class="gr">(formal)</span><p class="df">lasting or used for only a short time</p><ul><li>Fame in that world is ephemeral.</li></ul>'),
    run: ld('run', 'verb', ' <span class="gr">(ran, run)</span><p class="df"><span class="n">1</span>to move fast on foot, faster than walking</p><ul><li>I had to run to catch the last train.</li></ul><p class="df"><span class="n">2</span>to be in charge of a business or an organization</p><ul><li>They run a bakery near the station.</li></ul><p class="df"><span class="n">3</span>(of a machine or program) to operate or be working</p><ul><li>Leave the engine running.</li></ul><p class="df"><span class="n">4</span>(of a liquid) to flow in a particular direction</p><ul><li>Tears ran down her face.</li></ul><p class="df"><span class="n">5</span>to continue for a particular period of time</p><ul><li>The exhibition runs until the end of March.</li></ul><span class="pos">noun</span><p class="df"><span class="n">1</span>an act of running; a period of time spent running</p><ul><li>She goes for a run before breakfast.</li></ul><p class="df"><span class="n">2</span>a series of similar events that happen one after another</p><ul><li>The team has had a run of bad luck.</li></ul>'),
  },
  et: {
    serendipity: '<span class="yr">1754.</span> Coined by Horace Walpole in a letter to Horace Mann, from <i>Serendip</i>, an old name for Sri Lanka, + <i>-ity</i>; after the Persian tale <i>The Three Princes of Serendip</i>, whose heroes “were always making discoveries, by accidents and sagacity, of things which they were not in quest of”.',
    ephemeral: '<span class="yr">16th century.</span> From Greek <i>ephēmeros</i> “lasting only a day”, from <i>epi</i> “on” + <i>hēmera</i> “day”. First used of fevers and of short-lived insects and flowers; the general sense “short-lived” followed.',
    serif: '<span class="yr">Early 19th century.</span> Origin uncertain; perhaps from Dutch <i>schreef</i> “stroke, line”, from <i>schrijven</i> “to write”. Compare <i>sans serif</i>, recorded shortly afterwards.',
  },
  ej: {
    serendipity: ej('serendipity', '名', '〔U〕思いがけないものを偶然に発見する才能；掘り出し上手。<span class="x">▸ by serendipity　偶然の幸運によって</span><span class="x">▸ a moment of serendipity　思わぬ幸運の瞬間</span>'),
    serene: ej('serene', '形', '①静かな、穏やかな。②（空・天候が）晴れた、のどかな。<span class="x">▸ a serene smile　穏やかなほほえみ</span>'),
    series: ej('series', '名', '（複 series）①連続、ひと続き。②（番組・書物の）シリーズ。<span class="x">▸ a series of accidents　相次ぐ事故</span>'),
    ephemeral: ej('ephemeral', '形', 'つかの間の、はかない；一日限りの。<span class="x">▸ ephemeral beauty　はかない美しさ</span>'),
  },
  th: {
    serene: '<span class="hw">serene</span><div class="sense">calm</div><div class="syn">tranquil · placid · peaceful · untroubled · composed</div><div class="sense">clear</div><div class="syn">cloudless · bright · fair</div><div class="sense">see also</div><div class="syn"><a href="entry://serenity">serenity</a></div>',
    serious: '<span class="hw">serious</span><div class="sense">bad</div><div class="syn">grave · severe · critical · acute</div><div class="sense">not joking</div><div class="syn">earnest · solemn · sober · sincere</div>',
    ephemeral: '<span class="hw">ephemeral</span><div class="sense">short-lived</div><div class="syn">fleeting · transient · momentary · passing · brief</div>',
  },
  kk: {
    '木漏れ日': '<span class="hw">こもれび【木漏れ日】</span><span class="pos">〔名〕</span>樹木の枝葉のすきまから差しこむ日の光。<span class="x">「―が小道にゆれる」</span>',
    '食べる': '<span class="hw">たべる【食べる】</span><span class="pos">〔他下一〕</span><span class="s">①食物を口に入れ、かんで飲みこむ。<span class="x">「朝ごはんを―」</span></span><span class="s">②暮らしを立てる。生活する。<span class="x">「筆一本で―」</span></span>',
    '儚い': '<span class="hw">はかない【儚い・果敢ない】</span><span class="pos">〔形〕</span><span class="s">①消えやすく、長く続かない。<span class="x">「―命」</span></span><span class="s">②頼りにならない。むなしい。<span class="x">「―望み」</span></span>',
  },
  jc: {
    '木漏れ日': '<span class="hw">木漏れ日</span><span class="rd">こもれび</span><span class="ac">3</span><span class="pos">名</span><ol><li>从树叶缝隙间照进来的阳光<div class="ex">木漏れ日の中を歩く。在斑驳的树影里行走。</div></li></ol>',
    '食べる': '<span class="hw">食べる</span><span class="rd">たべる</span><span class="ac">2</span><span class="pos">他动・二类</span><ol><li>吃<div class="ex">朝ごはんを食べる。吃早饭。</div></li><li>生活；糊口<div class="ex">この給料では食べていけない。靠这点工资没法生活。</div></li></ol>',
    '儚い': '<span class="hw">儚い</span><span class="rd">はかない</span><span class="ac">3</span><span class="pos">形</span><ol><li>短暂的；无常的<div class="ex">儚い夢。一场短暂的梦。</div></li><li>靠不住的；虚幻的</li></ol>',
  },
  xh: {
    '邂逅': '<span class="hw">邂逅</span><span class="py">xièhòu</span>〈书〉动 没有约定而偶然相遇：<span class="x">与故人～｜一次美好的～。</span>',
    '须臾': '<span class="hw">须臾</span><span class="py">xūyú</span>〈书〉名 很短的一段时间；一会儿：<span class="x">～之间｜～不可分离。</span>',
  },
};

// What the page head shows beside a headword: how it is said, and its language.
export const HEAD = {
  serendipity: { say: '/ˌserənˈdɪpəti/', lang: 'en' }, serendipitous: { say: '/ˌserənˈdɪpətəs/', lang: 'en' },
  serene: { say: '/səˈriːn/', lang: 'en' }, serenity: { say: '/səˈrenəti/', lang: 'en' },
  serial: { say: '/ˈsɪəriəl/', lang: 'en' }, series: { say: '/ˈsɪəriːz/', lang: 'en' },
  serif: { say: '/ˈserɪf/', lang: 'en' }, serious: { say: '/ˈsɪəriəs/', lang: 'en' },
  ephemeral: { say: '/ɪˈfemərəl/', lang: 'en' }, run: { say: '/rʌn/', lang: 'en' },
  '木漏れ日': { say: 'こもれび', lang: 'ja' }, '食べる': { say: 'たべる', lang: 'ja' }, '儚い': { say: 'はかない', lang: 'ja' },
  '邂逅': { say: 'xièhòu', lang: 'zh' }, '须臾': { say: 'xūyú', lang: 'zh' },
};

// Inflected form -> base-form headword. Stands in for rule-based Lemmatization.
export const LEMMAS = {
  serendipities: 'serendipity', running: 'run', runs: 'run', ran: 'run', serials: 'serial',
  '食べた': '食べる', '食べます': '食べる', '食べて': '食べる', '食べない': '食べる',
  '儚く': '儚い', '儚かった': '儚い',
};

// Short sample stubs, so the headword column reads like a real index. All authored, like the rest.
const POS = { n: ['n.', 'noun'], v: ['v.', 'verb'], adj: ['adj.', 'adjective'], det: ['det.', 'determiner'] };
const EN_STUBS = `epic|n|史诗|a long poem about the deeds of heroes
epidemic|n|流行病|a disease that spreads quickly among many people
epigram|n|警句；隽语|a short, clever saying or poem
epilogue|n|尾声；后记|a short part added at the end of a book or play
episode|n|一集；插曲|one part of a story that is broadcast in several parts
epitaph|n|墓志铭|words written on a tomb in memory of the dead
epitome|n|典型；缩影|a perfect example of something
epoch|n|时代；纪元|a long period of history
equal|adj|相等的；平等的|the same in size, number or value
runaway|adj|逃跑的；失控的|having escaped, or no longer under control
rune|n|如尼字母|a letter of an ancient Germanic alphabet
rung|n|梯级；横档|one of the bars that form the steps of a ladder
runner|n|跑步者；赛跑选手|a person who runs, especially in a race
runny|adj|流鼻涕的；稀的|more liquid than usual
runway|n|跑道|a long strip of ground where aircraft take off and land
rupture|n|破裂；决裂|a sudden break or burst
rural|adj|乡村的；农村的|connected with the countryside rather than a town
ruse|n|诡计；计策|a trick intended to deceive someone
rush|v|匆忙；赶紧|to move or do something with great speed
rust|n|铁锈|the reddish-brown substance that forms on wet iron
rustic|adj|乡村的；质朴的|simple and typical of the countryside
rustle|v|沙沙作响|to make a soft sound like dry leaves moving
rut|n|车辙；刻板的生活|a deep track left by a wheel; a fixed, dull routine
ruthless|adj|无情的；冷酷的|hard and determined, showing no pity
sequel|n|续集；后续|a book or film that continues an earlier story
sequence|n|顺序；一连串|the order in which things happen or are arranged
sequester|v|使隔绝；扣押|to keep someone or something apart from others
seraph|n|六翼天使|an angel of the highest rank
serenade|n|小夜曲|a song or tune played outdoors at night for someone
serf|n|农奴|a farm worker in the past who was bound to the land
serfdom|n|农奴制|the system under which serfs lived and worked
serge|n|哔叽（一种毛料）|a strong woollen cloth used for clothing
sergeant|n|中士；警佐|a rank in the army or the police
sericulture|n|养蚕业|the raising of silkworms to produce silk
sermon|n|布道；说教|a talk on a moral or religious subject
serpent|n|蛇（尤指大蛇）|a snake, especially a large one
serrated|adj|锯齿状的|having a row of sharp points along the edge
serum|n|血清|the thin liquid part of blood
servant|n|仆人；佣人|a person who works in another person's house
serve|v|服务；供应|to give someone food or help; to work for someone
service|n|服务；业务|work done for others; a system that meets a public need
serviette|n|餐巾|a piece of cloth or paper used at meals
servile|adj|奴性的；卑躬屈膝的|too eager to obey others
session|n|会议；一段时间|a period of time spent on one activity
settle|v|解决；定居|to end a disagreement; to make a place your home
sever|v|切断；断绝|to cut something off; to end a connection completely
several|det|几个；数个|more than two but not many
severe|adj|严重的；严厉的|very bad or serious; strict
sew|v|缝；缝制|to join cloth using a needle and thread`;
for (const line of EN_STUBS.split('\n')) {
  const [hw, pos, zh, en] = line.split('|');
  ENTRIES.ec[hw] = `<span class="hw">${hw}</span><span class="pos">${POS[pos][0]}</span><ol><li>${zh}</li></ol>`;
  ENTRIES.ld[hw] = `<span class="hw">${hw}</span> <span class="pos">${POS[pos][1]}</span><p class="df">${en}</p>`;
}
const JA_STUBS = `食べ物|たべもの|名|食べるもの。食物。|食物
食卓|しょくたく|名|食事をするための台。|餐桌
食堂|しょくどう|名|食事をする部屋や店。|食堂；饭馆
食欲|しょくよく|名|食べたいと思う気持ち。|食欲
木陰|こかげ|名|木の下の、日の当たらない所。|树荫
木枯らし|こがらし|名|秋の終わりから冬にかけて吹く、冷たく強い風。|秋末冬初的寒风
木の葉|このは|名|樹木の葉。|树叶
木立|こだち|名|まとまって生えている木々。|树丛
儚む|はかなむ|動|はかないものと思う。|感到无常
墓|はか|名|死者を葬る所。|坟墓
計る|はかる|動|数量や時間などを調べる。|测量；计量
白紙|はくし|名|何も書いていない紙。|白纸
拍手|はくしゅ|名|手を打ち合わせて音を出すこと。|鼓掌
博物館|はくぶつかん|名|資料を集めて展示する施設。|博物馆`;
for (const line of JA_STUBS.split('\n')) {
  const [hw, reading, pos, ja, zh] = line.split('|');
  ENTRIES.kk[hw] = `<span class="hw">${reading}【${hw}】</span><span class="pos">〔${pos}〕</span>${ja}`;
  ENTRIES.jc[hw] = `<span class="hw">${hw}</span><span class="rd">${reading}</span><span class="pos">${pos === '名' ? '名' : '动'}</span><ol><li>${zh}</li></ol>`;
  HEAD[hw] = { say: reading, lang: 'ja' };
}
const ZH_STUBS = `协调|xiétiáo|形|配合得适当：|色彩～。
写作|xiězuò|动|写文章；创作文学作品：|从事～。
谢绝|xièjué|动|婉言拒绝：|～参观。
谢幕|xièmù|动|演出结束后，演员向观众致意：|再三～。
懈怠|xièdài|形|松懈懒散：|工作不可～。
虚心|xūxīn|形|不自以为是，肯接受意见：|～求教。
须要|xūyào|动|一定要：|做这项工作～细心。
须知|xūzhī|名|必须知道的事项：|考试～。
徐徐|xúxú|副|慢慢地：|列车～开动。
许多|xǔduō|数|很多：|～朋友。
序言|xùyán|名|写在著作正文之前的文章：|为新书作～。
叙述|xùshù|动|把事情的经过说出来或写出来：|～详尽。`;
for (const line of ZH_STUBS.split('\n')) {
  const [hw, pinyin, pos, def, example] = line.split('|');
  ENTRIES.xh[hw] = `<span class="hw">${hw}</span><span class="py">${pinyin}</span>${pos} ${def}<span class="x">${example}</span>`;
  HEAD[hw] = { say: pinyin, lang: 'zh' };
}

export const LOCALES = { zh: '简体中文', en: 'English', ja: '日本語' };
export const HTML_LANG = { zh: 'zh-CN', en: 'en', ja: 'ja' };

export const T = {
  zh: {
    group: '当前分组', back: '后退', forward: '前进', lookup: '查词', search_ph: '输入要查的词', cmd: '命令', close: '关闭',
    left_tabs: '左栏内容', suggest: '联想', history: '历史', wordbook: '生词本', library: '词库', rail: '词典跳转', page_label: '词条',
    sample: '样例', sample_note: '词条内容为示例，非真实词典',
    lemma: '形态还原', near: '没有完全匹配，以下是相近的词', answered: (n, all) => `${all} 本词典中 ${n} 本有此词条`, none_n: n => `${n} 本无此词条`,
    speak: '发音', speak_note: '原型用系统语音代替词典音频', save: '加入生词本', saved: '已在生词本', collapse: '折叠', expand: '展开',
    as_authored: '深色主题下按词典原样显示', no_group: hw => `当前分组里没有词典收录「${hw}」。`, nomatch: q => `没有找到「${q}」。`,
    list_empty_suggest: '没有匹配的词头', list_empty_history: '还没有查词记录', list_empty_wordbook: '生词本是空的',
    added: hw => `已把「${hw}」加入生词本`, removed: hw => `已把「${hw}」移出生词本`, dicts_n: n => `${n} 本词典`,
    tips: k => `↑ ↓ 选词，Enter 记入历史，${k} K 打开命令面板，Alt 加数字跳到第几本词典。`, recent: '最近查过', index: '词头表',
    miss_note: '没有匹配。词头表里接下来的词：', not_found: '没有找到', in_all: (g, hw) => `分组「${g}」里有「${hw}」`,
    wildcard_tip: '拿不准拼写时，用 * 代替任意几个字母，用 ? 代替一个字母。', try_q: '试试', next_words: '词头表里接下来的词',
    add_folder: '添加文件夹…', rescan: '重新扫描', new_group: '新建分组…', proto_only: '原型没有接文件系统：正式版会在这里打开系统对话框。',
    c_speak: '朗读当前词', c_back: '后退', c_forward: '前进', c_authored_all: '深色主题：全部词典按原样显示', c_dimmed_all: '深色主题：全部词典转成深色显示', toggle_side: '显示或隐藏左栏',
    lib_meta: n => `共 ${n} 本词典`, lib_folders: '词库文件夹', lib_found: n => `发现 ${n} 本词典`, lib_dicts: '词典',
    col_order: '顺序', col_on: '启用', col_name: '名称', col_entries: '词条数', col_size: '大小', move_up: '上移', move_down: '下移',
    enable: name => `启用 ${name}`, in_group: (name, g) => `${name} 属于分组 ${g}`, lib_note: '顺序决定查词结果里词典的先后。词条数和大小为示例数字。',
    palette_ph: '输入命令', cmd_title: '命令面板', pal_empty: '没有匹配的命令',
    c_focus: '聚焦搜索框', c_save: '把当前词加入生词本', c_unsave: '把当前词移出生词本', c_mini: '打开迷你窗口演示', c_library: '打开词库管理',
    c_lookup: '回到查词', c_history: '显示历史', c_wordbook: '显示生词本', c_fold_all: '折叠全部词典', c_unfold_all: '展开全部词典',
    c_group: name => `切换到分组「${name}」`, c_theme_system: cur => `主题：跟随系统（当前${cur}）`, c_theme_light: '主题：浅色', c_theme_dark: '主题：深色',
    light: '浅色', dark: '深色', c_lang: name => `界面语言：${name}`,
    mini_demo: '迷你窗口演示', mini_how: k => `点带虚线的词，或选中任意文字后按 ${k}+Alt+D`, mini_back: '回到主窗口', mini_open: '在主窗口打开',
    mini_none: q => `没有词典收录「${q}」。`, mini_group: '演示使用分组「全部」',
  },
  en: {
    group: 'Current Group', back: 'Back', forward: 'Forward', lookup: 'Look up', search_ph: 'Type a word', cmd: 'Commands', close: 'Close',
    left_tabs: 'Left column', suggest: 'Suggestions', history: 'History', wordbook: 'Word Book', library: 'Library', rail: 'Jump to dictionary', page_label: 'Entries',
    sample: 'sample', sample_note: 'Entries are samples, not from a real dictionary',
    lemma: 'Lemmatized', near: 'No exact match. Close headwords:', answered: (n, all) => `in ${n} of ${all} dictionaries`, none_n: n => `${n} without an entry`,
    speak: 'Pronounce', speak_note: 'The prototype uses system speech in place of dictionary audio', save: 'Add to Word Book', saved: 'In Word Book', collapse: 'Collapse', expand: 'Expand',
    as_authored: 'Show as authored in the dark theme', no_group: hw => `No dictionary in the Current Group has “${hw}”.`, nomatch: q => `Nothing found for “${q}”.`,
    list_empty_suggest: 'No matching headwords', list_empty_history: 'No lookups yet', list_empty_wordbook: 'The Word Book is empty',
    added: hw => `Added “${hw}” to the Word Book`, removed: hw => `Removed “${hw}” from the Word Book`, dicts_n: n => `${n} dictionaries`,
    tips: k => `↑ ↓ choose a word, Enter records it in History, ${k} K opens the command palette, Alt plus a digit jumps to that dictionary.`, recent: 'Recent lookups', index: 'Headword index',
    miss_note: 'No match. Next in the headword index:', not_found: 'not found', in_all: (g, hw) => `“${hw}” is in Group “${g}”`,
    wildcard_tip: 'Unsure of the spelling? Use * for any run of letters and ? for a single letter.', try_q: 'Try', next_words: 'Next in the headword index',
    add_folder: 'Add folder…', rescan: 'Rescan', new_group: 'New Group…', proto_only: 'The prototype is not connected to the file system. The app opens a system dialog here.',
    c_speak: 'Pronounce this word', c_back: 'Back', c_forward: 'Forward', c_authored_all: 'Dark theme: show every dictionary as authored', c_dimmed_all: 'Dark theme: recolour every dictionary', toggle_side: 'Show or hide the left column',
    lib_meta: n => `${n} dictionaries`, lib_folders: 'Library Folders', lib_found: n => `${n} dictionaries found`, lib_dicts: 'Dictionaries',
    col_order: 'Order', col_on: 'On', col_name: 'Name', col_entries: 'Entries', col_size: 'Size', move_up: 'Move up', move_down: 'Move down',
    enable: name => `Enable ${name}`, in_group: (name, g) => `${name} is in Group ${g}`, lib_note: 'Order sets the sequence of dictionaries in results. Entry counts and sizes are sample figures.',
    palette_ph: 'Type a command', cmd_title: 'Command palette', pal_empty: 'No matching command',
    c_focus: 'Focus the search field', c_save: 'Add this word to the Word Book', c_unsave: 'Remove this word from the Word Book', c_mini: 'Open the Mini Window demo', c_library: 'Open Library management',
    c_lookup: 'Return to Lookup', c_history: 'Show History', c_wordbook: 'Show Word Book', c_fold_all: 'Collapse all dictionaries', c_unfold_all: 'Expand all dictionaries',
    c_group: name => `Switch to Group “${name}”`, c_theme_system: cur => `Theme: follow system (currently ${cur})`, c_theme_light: 'Theme: light', c_theme_dark: 'Theme: dark',
    light: 'light', dark: 'dark', c_lang: name => `Interface language: ${name}`,
    mini_demo: 'Mini Window demo', mini_how: k => `Click a dotted word, or select any text and press ${k}+Alt+D`, mini_back: 'Back to main window', mini_open: 'Open in main window',
    mini_none: q => `No dictionary has “${q}”.`, mini_group: 'The demo uses the Group “全部”',
  },
  ja: {
    group: '現在のグループ', back: '戻る', forward: '進む', lookup: '検索', search_ph: '調べたい語を入力', cmd: 'コマンド', close: '閉じる',
    left_tabs: '左カラムの内容', suggest: '候補', history: '履歴', wordbook: '単語帳', library: 'ライブラリ', rail: '辞書へジャンプ', page_label: '項目',
    sample: '見本', sample_note: '項目の内容は見本であり、実在の辞書ではありません',
    lemma: '原形に変換', near: '完全一致はありません。近い見出し語：', answered: (n, all) => `${all} 冊中 ${n} 冊に項目があります`, none_n: n => `${n} 冊は項目なし`,
    speak: '発音', speak_note: '試作ではシステム音声で辞書音声を代用しています', save: '単語帳に追加', saved: '単語帳に登録済み', collapse: '折りたたむ', expand: '展開する',
    as_authored: 'ダークテーマでも辞書本来の表示にする', no_group: hw => `現在のグループには「${hw}」を収録した辞書がありません。`, nomatch: q => `「${q}」は見つかりませんでした。`,
    list_empty_suggest: '一致する見出し語がありません', list_empty_history: '検索履歴はまだありません', list_empty_wordbook: '単語帳は空です',
    added: hw => `「${hw}」を単語帳に追加しました`, removed: hw => `「${hw}」を単語帳から削除しました`, dicts_n: n => `辞書 ${n} 冊`,
    tips: k => `↑ ↓ で語を選択、Enter で履歴に記録、${k} K でコマンドパレット、Alt と数字で該当の辞書へ移動。`, recent: '最近調べた語', index: '見出し語一覧',
    miss_note: '一致なし。見出し語一覧の続き：', not_found: '見つかりません', in_all: (g, hw) => `グループ「${g}」に「${hw}」があります`,
    wildcard_tip: 'つづりが不確かなときは、* で任意の文字列、? で一文字を表せます。', try_q: '例', next_words: '見出し語一覧の続き',
    add_folder: 'フォルダを追加…', rescan: '再スキャン', new_group: 'グループを作成…', proto_only: '試作ではファイルシステムに接続していません。製品版ではここでシステムのダイアログが開きます。',
    c_speak: 'この語を発音', c_back: '戻る', c_forward: '進む', c_authored_all: 'ダークテーマ：すべての辞書を本来の表示にする', c_dimmed_all: 'ダークテーマ：すべての辞書を暗色に変換する', toggle_side: '左カラムの表示切り替え',
    lib_meta: n => `辞書 ${n} 冊`, lib_folders: 'ライブラリフォルダ', lib_found: n => `辞書 ${n} 冊を検出`, lib_dicts: '辞書',
    col_order: '順序', col_on: '有効', col_name: '名称', col_entries: '項目数', col_size: 'サイズ', move_up: '上へ', move_down: '下へ',
    enable: name => `${name} を有効にする`, in_group: (name, g) => `${name} をグループ ${g} に含める`, lib_note: '順序は検索結果での辞書の並びを決めます。項目数とサイズは見本の数値です。',
    palette_ph: 'コマンドを入力', cmd_title: 'コマンドパレット', pal_empty: '一致するコマンドがありません',
    c_focus: '検索欄にフォーカス', c_save: 'この語を単語帳に追加', c_unsave: 'この語を単語帳から削除', c_mini: 'ミニウィンドウのデモを開く', c_library: '辞書ライブラリの管理を開く',
    c_lookup: '検索に戻る', c_history: '履歴を表示', c_wordbook: '単語帳を表示', c_fold_all: 'すべての辞書を折りたたむ', c_unfold_all: 'すべての辞書を展開する',
    c_group: name => `グループ「${name}」に切り替え`, c_theme_system: cur => `テーマ：システムに合わせる（現在は${cur}）`, c_theme_light: 'テーマ：ライト', c_theme_dark: 'テーマ：ダーク',
    light: 'ライト', dark: 'ダーク', c_lang: name => `表示言語：${name}`,
    mini_demo: 'ミニウィンドウのデモ', mini_how: k => `点線の語をクリックするか、文字を選択して ${k}+Alt+D`, mini_back: 'メインウィンドウへ戻る', mini_open: 'メインウィンドウで開く',
    mini_none: q => `「${q}」を収録した辞書がありません。`, mini_group: 'デモではグループ「全部」を使用',
  },
};
