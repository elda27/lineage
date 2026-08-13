import { FormEvent, useMemo, useState } from "react";
import { UpdateBanner } from "./updater/UpdateBanner";
import "./App.css";

type IconName =
  | "home" | "search" | "sparkles" | "settings" | "plus" | "clock"
  | "check" | "file" | "arrow" | "more" | "tag" | "trash" | "edit"
  | "command" | "inbox" | "chevron" | "play" | "moon";

function Icon({ name, size = 18 }: { name: IconName; size?: number }) {
  const paths: Record<IconName, React.ReactNode> = {
    home: <><path d="m3 11 9-8 9 8"/><path d="M5 10v10h14V10M9 20v-6h6v6"/></>,
    search: <><circle cx="11" cy="11" r="7"/><path d="m20 20-4-4"/></>,
    sparkles: <><path d="m12 3 1.2 3.8L17 8l-3.8 1.2L12 13l-1.2-3.8L7 8l3.8-1.2L12 3Z"/><path d="m5 14 .8 2.2L8 17l-2.2.8L5 20l-.8-2.2L2 17l2.2-.8L5 14ZM19 13l.6 1.4L21 15l-1.4.6L19 17l-.6-1.4L17 15l1.4-.6L19 13Z"/></>,
    settings: <><circle cx="12" cy="12" r="3"/><path d="M19.4 15a1.7 1.7 0 0 0 .3 1.9l.1.1-2.8 2.8-.1-.1a1.7 1.7 0 0 0-1.9-.3 1.7 1.7 0 0 0-1 1.6v.2h-4V21a1.7 1.7 0 0 0-1-1.6 1.7 1.7 0 0 0-1.9.3l-.1.1L4.2 17l.1-.1a1.7 1.7 0 0 0 .3-1.9A1.7 1.7 0 0 0 3 14H2.8v-4H3a1.7 1.7 0 0 0 1.6-1 1.7 1.7 0 0 0-.3-1.9L4.2 7 7 4.2l.1.1a1.7 1.7 0 0 0 1.9.3A1.7 1.7 0 0 0 10 3v-.2h4V3a1.7 1.7 0 0 0 1 1.6 1.7 1.7 0 0 0 1.9-.3l.1-.1L19.8 7l-.1.1a1.7 1.7 0 0 0-.3 1.9 1.7 1.7 0 0 0 1.6 1h.2v4H21a1.7 1.7 0 0 0-1.6 1Z"/></>,
    plus: <path d="M12 5v14M5 12h14"/>, clock: <><circle cx="12" cy="12" r="9"/><path d="M12 7v5l3 2"/></>,
    check: <path d="m5 12 4 4L19 6"/>, file: <><path d="M6 3h8l4 4v14H6z"/><path d="M14 3v5h5M9 13h6M9 17h5"/></>,
    arrow: <><path d="M5 12h14M14 7l5 5-5 5"/></>, more: <><circle cx="5" cy="12" r="1" fill="currentColor"/><circle cx="12" cy="12" r="1" fill="currentColor"/><circle cx="19" cy="12" r="1" fill="currentColor"/></>,
    tag: <><path d="M20 13 13 20 4 11V4h7z"/><circle cx="8.5" cy="8.5" r="1"/></>, trash: <><path d="M4 7h16M9 7V4h6v3M7 7l1 14h8l1-14M10 11v6M14 11v6"/></>,
    edit: <><path d="m14 5 5 5L9 20H4v-5zM12 7l5 5"/></>, command: <><rect x="4" y="4" width="6" height="6" rx="3"/><rect x="14" y="4" width="6" height="6" rx="3"/><rect x="4" y="14" width="6" height="6" rx="3"/><rect x="14" y="14" width="6" height="6" rx="3"/><path d="M10 7v10c0 4 4 4 4 0V7"/></>,
    inbox: <><path d="M4 5h16v14H4z"/><path d="M4 14h5l2 2h2l2-2h5"/></>, chevron: <path d="m9 7 5 5-5 5"/>,
    play: <path d="m9 6 9 6-9 6z"/>, moon: <path d="M20 15.5A8 8 0 0 1 8.5 4 8.5 8.5 0 1 0 20 15.5Z"/>,
  };
  return <svg className="icon" width={size} height={size} viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1.8" strokeLinecap="round" strokeLinejoin="round" aria-hidden>{paths[name]}</svg>;
}

type Page = "home" | "search" | "automation" | "settings";
type Memo = { id: number; title: string; body: string; tags: string[]; time: string; type: "task" | "memo"; done?: boolean };

const initialMemos: Memo[] = [
  { id: 1, title: "プロジェクトの方向性を整理する", body: "次回のミーティングまでに、ロードマップと優先順位をまとめる。", tags: ["仕事", "企画"], time: "10分前", type: "task" },
  { id: 2, title: "デザインリサーチのメモ", body: "余白を広く取り、情報の階層を明確にする。操作はできるだけ軽やかに。", tags: ["アイデア", "デザイン"], time: "1時間前", type: "memo" },
  { id: 3, title: "請求書を送付", body: "8月分の請求書を確認してクライアントへ送る。", tags: ["タスク", "仕事"], time: "昨日", type: "task" },
  { id: 4, title: "読みたい本", body: "The Design of Everyday Things / ドン・ノーマン", tags: ["個人", "読書"], time: "8月10日", type: "memo" },
  { id: 5, title: "週末の買い物リスト", body: "コーヒー豆、観葉植物の土、電球。", tags: ["生活"], time: "8月9日", type: "memo" },
];

function Sidebar({ page, setPage }: { page: Page; setPage: (p: Page) => void }) {
  const items: { page: Page; label: string; icon: IconName }[] = [
    { page: "home", label: "ホーム", icon: "home" }, { page: "search", label: "検索", icon: "search" },
    { page: "automation", label: "自動化", icon: "sparkles" }, { page: "settings", label: "設定", icon: "settings" },
  ];
  return <aside className="sidebar">
    <button className="brand" onClick={() => setPage("home")}><span className="brand-mark">L</span><span>lineage</span></button>
    <nav>{items.map(item => <button key={item.page} className={page === item.page ? "active" : ""} onClick={() => setPage(item.page)}><Icon name={item.icon}/><span>{item.label}</span>{item.page === "search" && <kbd>⌘ K</kbd>}</button>)}</nav>
    <div className="sidebar-foot"><div className="storage"><div><span>ローカルストレージ</span><span>24 MB / 1 GB</span></div><div className="meter"><i/></div></div><button className="profile"><span className="avatar">Y</span><span><b>山田 太郎</b><small>ローカルワークスペース</small></span><Icon name="more"/></button></div>
  </aside>;
}

function SearchBox({ value, onChange, onSubmit, large = false }: { value: string; onChange: (v:string)=>void; onSubmit?:()=>void; large?: boolean }) {
  return <div className={`search-box ${large ? "large" : ""}`}><Icon name="search"/><input aria-label="メモを検索" value={value} onChange={e=>onChange(e.target.value)} onKeyDown={e=>e.key === "Enter" && onSubmit?.()} placeholder="メモやタスクを検索…"/><span className="shortcut">⌘ K</span></div>;
}

function MemoCard({ memo, onOpen, onToggle }: { memo: Memo; onOpen:()=>void; onToggle:()=>void }) {
  return <article className="memo-card" onClick={onOpen} tabIndex={0} onKeyDown={e=>e.key === "Enter" && onOpen()}>
    <div className={`type-icon ${memo.type}`}><Icon name={memo.type === "task" ? "check" : "file"}/></div>
    <div className="memo-content"><div className="memo-title"><h3 className={memo.done ? "done" : ""}>{memo.title}</h3>{memo.type === "task" && <button className={`check-button ${memo.done ? "checked" : ""}`} aria-label="完了状態を切り替え" onClick={e=>{e.stopPropagation(); onToggle();}}>{memo.done && <Icon name="check" size={13}/>}</button>}</div><p>{memo.body}</p><div className="meta-row">{memo.tags.map(tag=><span className="tag" key={tag}>#{tag}</span>)}<span className="time"><Icon name="clock" size={13}/>{memo.time}</span></div></div>
    <button className="card-arrow" aria-label="詳細を開く"><Icon name="chevron"/></button>
  </article>;
}

function Home({ memos, setPage, openMemo, toggleMemo, createMemo }: { memos:Memo[]; setPage:(p:Page)=>void; openMemo:(m:Memo)=>void; toggleMemo:(id:number)=>void; createMemo:()=>void }) {
  const [query,setQuery] = useState("");
  return <div className="page home-page"><header className="topbar"><div/><button className="quiet" aria-label="テーマ設定を開く" onClick={()=>setPage("settings")}><Icon name="moon" size={17}/></button><button className="primary small" onClick={createMemo}><Icon name="plus" size={16}/>新しいメモ</button></header>
    <div className="hero"><p className="eyebrow">2026年8月12日 水曜日</p><h1>おかえりなさい、山田さん。</h1><p>思考の続きを、ここから始めましょう。</p><SearchBox large value={query} onChange={setQuery} onSubmit={()=>setPage("search")}/></div>
    <section className="dashboard"><div className="section-heading"><div><h2>最近の記録</h2><p>新しく追加・更新されたメモ</p></div><button className="text-button" onClick={()=>setPage("search")}>すべて表示 <Icon name="arrow" size={15}/></button></div>
      <div className="memo-list">{memos.slice(0,4).map(m=><MemoCard memo={m} key={m.id} onOpen={()=>openMemo(m)} onToggle={()=>toggleMemo(m.id)}/>)}</div>
      <div className="quick-grid"><button onClick={()=>setPage("search")}><span className="quick-icon lavender"><Icon name="inbox"/></span><span><b>すべての記録</b><small>{memos.length} 件のメモとタスク</small></span><Icon name="arrow"/></button><button onClick={()=>setPage("automation")}><span className="quick-icon mint"><Icon name="sparkles"/></span><span><b>自動化ルール</b><small>2 件が有効</small></span><Icon name="arrow"/></button></div>
    </section></div>;
}

function SearchPage({ memos, openMemo, toggleMemo }: { memos:Memo[]; openMemo:(m:Memo)=>void; toggleMemo:(id:number)=>void }) {
  const [query,setQuery]=useState(""); const [filter,setFilter]=useState("すべて");
  const results=useMemo(()=>memos.filter(m=>(filter==="すべて" || (filter==="タスク" ? m.type==="task" : m.type==="memo")) && `${m.title} ${m.body} ${m.tags.join(" ")}`.toLowerCase().includes(query.toLowerCase())),[memos,query,filter]);
  return <div className="page standard-page"><div className="page-header"><p className="eyebrow">LIBRARY</p><h1>検索</h1><p>これまでに残したすべての記録を探せます。</p></div><SearchBox large value={query} onChange={setQuery}/><div className="filter-row">{["すべて","メモ","タスク"].map(f=><button className={filter===f?"selected":""} onClick={()=>setFilter(f)} key={f}>{f}</button>)}<span>{results.length} 件</span></div><div className="memo-list search-results">{results.map(m=><MemoCard key={m.id} memo={m} onOpen={()=>openMemo(m)} onToggle={()=>toggleMemo(m.id)}/>)}</div>{!results.length&&<div className="empty"><Icon name="search" size={30}/><h3>一致する記録がありません</h3><p>キーワードや絞り込みを変えてみてください。</p></div>}</div>;
}

function Automation() { const [rules,setRules]=useState([{name:"タスクを自動でまとめる",desc:"#タスク が付いた記録を今日のタスクリストへ追加",on:true},{name:"アイデアを週報へ整理",desc:"毎週金曜日に #アイデア の記録を要約",on:true},{name:"読書メモをアーカイブ",desc:"読み終えた本のメモを月末にアーカイブ",on:false}]); return <div className="page standard-page"><div className="page-header row-header"><div><p className="eyebrow">WORKFLOWS</p><h1>自動化</h1><p>記録に基づくルールで、いつもの作業を軽くします。</p></div><button className="primary"><Icon name="plus"/>新しいルール</button></div><div className="feature-card"><span><Icon name="sparkles" size={26}/></span><div><small>LINEAGE AGENT</small><h2>記録から、次のアクションへ。</h2><p>メモに含まれる文脈やメタ情報を読み取り、あなたに代わって整理・実行します。</p></div><button className="secondary"><Icon name="play" size={15}/>Agentを試す</button></div><h2 className="subheading">自動化ルール</h2><div className="rule-list">{rules.map((rule,i)=><article key={rule.name}><span className="rule-icon"><Icon name="command"/></span><div><h3>{rule.name}</h3><p>{rule.desc}</p><small>{rule.on?"有効・最終実行 2時間前":"停止中"}</small></div><button aria-label={`${rule.name}を切り替え`} className={`toggle ${rule.on?"on":""}`} onClick={()=>setRules(r=>r.map((v,n)=>n===i?{...v,on:!v.on}:v))}><i/></button><button className="quiet"><Icon name="more"/></button></article>)}</div></div> }

function SettingsPage() { const [launch,setLaunch]=useState(true),[copy,setCopy]=useState(true),[theme,setTheme]=useState("システム"); return <div className="page standard-page settings-page"><div className="page-header"><p className="eyebrow">PREFERENCES</p><h1>設定</h1><p>minos と fullos の動作をカスタマイズします。</p></div><section><h2>minos</h2><p>クイック入力の動作</p><div className="settings-card"><SettingRow title="システム起動時に開始" desc="PCへのログイン時にminosを自動で起動します"><Toggle value={launch} setValue={setLaunch}/></SettingRow><SettingRow title="直前のアプリからテキストを取得" desc="呼び出したとき、選択中のテキストを入力欄へコピーします"><Toggle value={copy} setValue={setCopy}/></SettingRow><SettingRow title="呼び出しショートカット" desc="どこからでもクイック入力を開きます"><kbd className="keybinding">Alt + Space</kbd></SettingRow></div></section><section><h2>表示</h2><p>fullos の見た目</p><div className="settings-card"><SettingRow title="テーマ" desc="アプリ全体の表示テーマ"><div className="segment">{["ライト","システム","ダーク"].map(v=><button className={theme===v?"active":""} onClick={()=>setTheme(v)} key={v}>{v}</button>)}</div></SettingRow></div></section><section><h2>メタ情報の短縮入力</h2><p>minos で # に続けて入力する短縮文字</p><div className="settings-card tags-setting">{[["タスク","task","32回使用"],["仕事","work","24回使用"],["アイデア","idea","18回使用"]].map(t=><div key={t[0]}><span className="tag">#{t[0]}</span><span className="short-value">{t[1]}</span><small>{t[2]}</small><button className="quiet"><Icon name="edit"/></button></div>)}</div></section></div> }
function SettingRow({title,desc,children}:{title:string;desc:string;children:React.ReactNode}){return <div className="setting-row"><div><b>{title}</b><small>{desc}</small></div>{children}</div>}
function Toggle({value,setValue}:{value:boolean;setValue:(v:boolean)=>void}){return <button role="switch" aria-checked={value} className={`toggle ${value?"on":""}`} onClick={()=>setValue(!value)}><i/></button>}

function Detail({ memo, close, update, remove }: { memo:Memo; close:()=>void; update:(m:Memo)=>void; remove:()=>void }) { const [editing,setEditing]=useState(false),[title,setTitle]=useState(memo.title),[body,setBody]=useState(memo.body); const save=(e:FormEvent)=>{e.preventDefault();update({...memo,title,body});setEditing(false)}; return <div className="overlay" onMouseDown={e=>e.target===e.currentTarget&&close()}><aside className="detail-panel"><header><button className="back-button" onClick={close}>← 戻る</button><div><button className="quiet" onClick={()=>setEditing(true)}><Icon name="edit"/></button><button className="quiet danger" onClick={remove}><Icon name="trash"/></button></div></header><div className="detail-icon"><Icon name={memo.type==="task"?"check":"file"}/></div>{editing?<form onSubmit={save} className="edit-form"><input autoFocus value={title} onChange={e=>setTitle(e.target.value)}/><textarea value={body} onChange={e=>setBody(e.target.value)}/><div><button type="button" className="secondary" onClick={()=>setEditing(false)}>キャンセル</button><button className="primary">保存する</button></div></form>:<><h1>{memo.title}</h1><p className="detail-body">{memo.body}</p></>}<div className="detail-info"><div><span>作成日時</span><b>2026年8月12日 10:24</b></div><div><span>種類</span><b>{memo.type==="task"?"タスク":"メモ"}</b></div><div><span>メタ情報</span><div>{memo.tags.map(t=><span className="tag" key={t}>#{t}</span>)}</div></div></div></aside></div> }

export default function App() { const [page,setPage]=useState<Page>("home"),[memos,setMemos]=useState(initialMemos),[selected,setSelected]=useState<Memo|null>(null); const toggle=(id:number)=>setMemos(m=>m.map(v=>v.id===id?{...v,done:!v.done}:v)); const create=()=>setSelected({id:Date.now(),title:"無題のメモ",body:"ここに内容を入力してください。",tags:[],time:"たった今",type:"memo"}); return <div className="app-shell"><UpdateBanner/><Sidebar page={page} setPage={setPage}/><main className="main-content">{page==="home"&&<Home memos={memos} setPage={setPage} openMemo={setSelected} toggleMemo={toggle} createMemo={create}/>} {page==="search"&&<SearchPage memos={memos} openMemo={setSelected} toggleMemo={toggle}/>} {page==="automation"&&<Automation/>} {page==="settings"&&<SettingsPage/>}</main>{selected&&<Detail memo={selected} close={()=>setSelected(null)} update={m=>{setMemos(v=>v.some(x=>x.id===m.id)?v.map(x=>x.id===m.id?m:x):[m,...v]);setSelected(m)}} remove={()=>{setMemos(v=>v.filter(x=>x.id!==selected.id));setSelected(null)}}/>}</div> }
