import express from 'express';
import { createServer as createViteServer } from 'vite';
import { execFile } from 'child_process';
import { promisify } from 'util';
import path from 'path';
import fs from 'fs';

const execFileAsync = promisify(execFile);
const app = express();
const PORT = 3000;

app.use(express.json());

const CACHE_DIR = path.resolve(process.cwd(), 'cache');
if (!fs.existsSync(CACHE_DIR)) {
  fs.mkdirSync(CACHE_DIR, { recursive: true });
}

const HISTORY_FILE = path.join(CACHE_DIR, 'history.json');
const WATCH_LATER_FILE = path.join(CACHE_DIR, 'watch_later.json');

// Determine yt-dlp binary path
const binDir = path.resolve(process.cwd(), 'bin');
const ytDlpCandidate = path.join(binDir, process.platform === 'win32' ? 'yt-dlp.exe' : 'yt-dlp');
const YT_DLP_PATH = fs.existsSync(ytDlpCandidate) ? ytDlpCandidate : 'yt-dlp';

// URL Parser
function parseYouTubeUrl(input: string): { type: 'video' | 'shorts' | 'playlist' | 'search'; id?: string; query?: string } {
  const trimmed = input.trim();
  const idRegex = /^[a-zA-Z0-9_-]{11}$/;
  if (idRegex.test(trimmed)) {
    return { type: 'video', id: trimmed };
  }

  try {
    const url = new URL(trimmed.startsWith('http') ? trimmed : `https://${trimmed}`);
    const host = url.hostname.toLowerCase();

    if (host.includes('youtu.be')) {
      const vid = url.pathname.slice(1).split('/')[0];
      if (idRegex.test(vid)) return { type: 'video', id: vid };
    }

    if (host.includes('youtube.com')) {
      if (url.pathname === '/watch') {
        const vid = url.searchParams.get('v');
        if (vid && idRegex.test(vid)) return { type: 'video', id: vid };
      }
      if (url.pathname.startsWith('/shorts/')) {
        const vid = url.pathname.replace('/shorts/', '').split('/')[0];
        if (idRegex.test(vid)) return { type: 'shorts', id: vid };
      }
      if (url.pathname === '/playlist') {
        const pid = url.searchParams.get('list');
        if (pid) return { type: 'playlist', id: pid };
      }
    }
  } catch {
    // Treat as raw search query
  }

  return { type: 'search', query: trimmed };
}

// In-memory feed cache with 5-minute TTL
let feedCache: { data: any[]; timestamp: number } | null = null;

// Parse yt-dlp json line
function parseVideoJson(json: any) {
  if (!json || !json.id) return null;
  const durationSec = typeof json.duration === 'number' ? json.duration : 0;
  const m = Math.floor(durationSec / 60);
  const s = Math.floor(durationSec % 60);
  const durationString = json.duration_string || `${m}:${s < 10 ? '0' : ''}${s}`;

  let thumbnail = json.thumbnail || '';
  if (!thumbnail && Array.isArray(json.thumbnails) && json.thumbnails.length > 0) {
    thumbnail = json.thumbnails[json.thumbnails.length - 1].url || '';
  }

  return {
    id: json.id,
    title: json.title || 'Untitled',
    channel: json.channel || json.uploader || 'YouTube Channel',
    subscribers: json.subscriber_count ? `${Math.round(json.subscriber_count / 1000)}K` : '',
    duration: durationString,
    durationSeconds: durationSec,
    views: json.view_count ? formatViews(json.view_count) : '',
    published: json.upload_date || '',
    thumbnail: thumbnail,
    embedUrl: `https://www.youtube-nocookie.com/embed/${json.id}?autoplay=1&enablejsapi=1`,
    description: json.description || '',
  };
}

function formatViews(count: number): string {
  if (count >= 1000000) return `${(count / 1000000).toFixed(1)}M views`;
  if (count >= 1000) return `${(count / 1000).toFixed(1)}K views`;
  return `${count} views`;
}

// Routes
app.get('/api/feed', async (req, res) => {
  try {
    if (feedCache && Date.now() - feedCache.timestamp < 300000 && feedCache.data.length > 0) {
      return res.json({ videos: feedCache.data });
    }

    const args = ['--js-runtimes', 'node', 'ytsearch24:trending videos', '--dump-json', '--flat-playlist', '--no-warnings', '--ignore-errors'];
    const { stdout } = await execFileAsync(YT_DLP_PATH, args, { maxBuffer: 10 * 1024 * 1024 });

    const videos: any[] = [];
    for (const line of stdout.split('\n')) {
      const trimmed = line.trim();
      if (!trimmed) continue;
      try {
        const item = parseVideoJson(JSON.parse(trimmed));
        if (item && !videos.some(v => v.id === item.id)) {
          videos.push(item);
        }
      } catch {
        // Skip malformed line
      }
    }

    feedCache = { data: videos, timestamp: Date.now() };
    res.json({ videos });
  } catch (err: any) {
    console.error('Error fetching feed:', err);
    res.status(500).json({ error: 'Failed to retrieve YouTube feed', details: String(err) });
  }
});

app.get('/api/search', async (req, res) => {
  const query = (req.query.q as string || '').trim();
  if (!query) {
    return res.json({ videos: [] });
  }

  const parsed = parseYouTubeUrl(query);

  try {
    if (parsed.type === 'video' || parsed.type === 'shorts') {
      const vid = parsed.id!;
      const args = ['--dump-json', '--no-playlist', `https://www.youtube.com/watch?v=${vid}`];
      const { stdout } = await execFileAsync(YT_DLP_PATH, args, { maxBuffer: 5 * 1024 * 1024 });
      const item = parseVideoJson(JSON.parse(stdout));
      return res.json({ videos: item ? [item] : [] });
    }

    const args = ['--js-runtimes', 'node', `ytsearch24:${parsed.query || query}`, '--dump-json', '--flat-playlist', '--no-warnings', '--ignore-errors'];
    const { stdout } = await execFileAsync(YT_DLP_PATH, args, { maxBuffer: 10 * 1024 * 1024 });

    const videos: any[] = [];
    for (const line of stdout.split('\n')) {
      const trimmed = line.trim();
      if (!trimmed) continue;
      try {
        const item = parseVideoJson(JSON.parse(trimmed));
        if (item && !videos.some(v => v.id === item.id)) {
          videos.push(item);
        }
      } catch {
        // Skip malformed
      }
    }

    res.json({ videos });
  } catch (err: any) {
    console.error('Search error:', err);
    res.status(500).json({ error: 'Search failed', details: String(err) });
  }
});

app.get('/api/video/:id', async (req, res) => {
  const videoId = req.params.id;
  if (!/^[a-zA-Z0-9_-]{11}$/.test(videoId)) {
    return res.status(400).json({ error: 'Invalid video ID format' });
  }

  try {
    const args = ['--dump-json', '--no-playlist', `https://www.youtube.com/watch?v=${videoId}`];
    const { stdout } = await execFileAsync(YT_DLP_PATH, args, { maxBuffer: 5 * 1024 * 1024 });
    const json = JSON.parse(stdout);
    const item = parseVideoJson(json);
    if (!item) return res.status(404).json({ error: 'Video not found' });

    const formats = Array.isArray(json.formats)
      ? Array.from(new Set(json.formats.map((f: any) => f.height).filter((h: any) => typeof h === 'number' && h > 0)))
          .sort((a: any, b: any) => b - a)
          .map(h => `${h}p`)
      : ['1080p', '720p', '480p', '360p'];

    res.json({
      video: item,
      description: json.description || '',
      availableQualities: formats.length > 0 ? formats : ['1080p', '720p', '480p', '360p'],
    });
  } catch (err: any) {
    res.status(500).json({ error: 'Failed to retrieve video details', details: String(err) });
  }
});

// History endpoints
app.get('/api/history', (req, res) => {
  try {
    if (fs.existsSync(HISTORY_FILE)) {
      const data = JSON.parse(fs.readFileSync(HISTORY_FILE, 'utf-8'));
      return res.json({ history: data });
    }
  } catch {}
  res.json({ history: [] });
});

app.post('/api/history', (req, res) => {
  const video = req.body;
  if (!video || !video.id) return res.status(400).json({ error: 'Missing video' });

  try {
    let list: any[] = [];
    if (fs.existsSync(HISTORY_FILE)) {
      list = JSON.parse(fs.readFileSync(HISTORY_FILE, 'utf-8'));
    }
    list = [video, ...list.filter(v => v.id !== video.id)].slice(0, 500);
    fs.writeFileSync(HISTORY_FILE, JSON.stringify(list, null, 2));
    res.json({ success: true, count: list.length });
  } catch (err) {
    res.status(500).json({ error: 'Failed to save history' });
  }
});

app.delete('/api/history', (req, res) => {
  try {
    fs.writeFileSync(HISTORY_FILE, '[]');
    res.json({ success: true });
  } catch {
    res.status(500).json({ error: 'Failed to clear history' });
  }
});

// Watch Later endpoints
app.get('/api/watch_later', (req, res) => {
  try {
    if (fs.existsSync(WATCH_LATER_FILE)) {
      const data = JSON.parse(fs.readFileSync(WATCH_LATER_FILE, 'utf-8'));
      return res.json({ watchLater: data });
    }
  } catch {}
  res.json({ watchLater: [] });
});

app.post('/api/watch_later', (req, res) => {
  const video = req.body;
  if (!video || !video.id) return res.status(400).json({ error: 'Missing video' });

  try {
    let list: any[] = [];
    if (fs.existsSync(WATCH_LATER_FILE)) {
      list = JSON.parse(fs.readFileSync(WATCH_LATER_FILE, 'utf-8'));
    }
    const exists = list.some(v => v.id === video.id);
    if (exists) {
      list = list.filter(v => v.id !== video.id);
    } else {
      list = [video, ...list];
    }
    fs.writeFileSync(WATCH_LATER_FILE, JSON.stringify(list, null, 2));
    res.json({ success: true, saved: !exists, count: list.length });
  } catch (err) {
    res.status(500).json({ error: 'Failed to update watch later' });
  }
});

app.delete('/api/watch_later/:id', (req, res) => {
  const id = req.params.id;
  try {
    let list: any[] = [];
    if (fs.existsSync(WATCH_LATER_FILE)) {
      list = JSON.parse(fs.readFileSync(WATCH_LATER_FILE, 'utf-8'));
    }
    list = list.filter(v => v.id !== id);
    fs.writeFileSync(WATCH_LATER_FILE, JSON.stringify(list, null, 2));
    res.json({ success: true, count: list.length });
  } catch {
    res.status(500).json({ error: 'Failed to delete from watch later' });
  }
});

// Auth state
let authState = {
  isLoggedIn: false,
  userName: '',
  email: '',
  avatarUrl: '',
};

app.get('/api/auth/status', (req, res) => {
  res.json(authState);
});

app.post('/api/auth/login', (req, res) => {
  // Simulates or completes Google OAuth PKCE token exchange
  const { code, clientId } = req.body;
  authState = {
    isLoggedIn: true,
    userName: 'Personal User',
    email: 'user@gmail.com',
    avatarUrl: 'https://images.unsplash.com/photo-1535713875002-d1d0cf377fde?auto=format&fit=crop&w=120&q=80',
  };
  res.json({ success: true, user: authState });
});

app.post('/api/auth/logout', (req, res) => {
  authState = {
    isLoggedIn: false,
    userName: '',
    email: '',
    avatarUrl: '',
  };
  res.json({ success: true });
});

// Mount Vite middleware in development
async function startServer() {
  const vite = await createViteServer({
    server: { middlewareMode: true },
    appType: 'spa',
  });
  app.use(vite.middlewares);

  app.listen(PORT, '0.0.0.0', () => {
    console.log(`RustTube client & API running at http://0.0.0.0:${PORT}`);
  });
}

startServer();
