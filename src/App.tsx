/**
 * @license
 * SPDX-License-Identifier: Apache-2.0
 */

import React, { useState, useEffect, useRef, useCallback } from 'react';
import {
  Play,
  Pause,
  RotateCcw,
  RotateCw,
  Volume2,
  VolumeX,
  Maximize2,
  Minimize2,
  Search,
  Settings,
  Clock,
  History,
  Tv,
  ListVideo,
  FolderTree,
  Bookmark,
  BookmarkCheck,
  CheckCircle2,
  UserCheck,
  LogOut,
  AlertCircle,
  Loader2,
  ThumbsUp,
  X,
  ExternalLink
} from 'lucide-react';

interface VideoItem {
  id: string;
  title: string;
  channel: string;
  subscribers?: string;
  duration: string;
  durationSeconds: number;
  views: string;
  published: string;
  thumbnail: string;
  embedUrl: string;
  description?: string;
}

interface UserAuth {
  isLoggedIn: boolean;
  userName: string;
  email: string;
  avatarUrl: string;
}

export default function App() {
  const [currentPage, setCurrentPage] = useState<'home' | 'watch' | 'subscriptions' | 'playlists' | 'watch_later' | 'history' | 'settings'>('home');
  const [searchQuery, setSearchQuery] = useState('');
  const [isLoading, setIsLoading] = useState(false);
  const [errorMessage, setErrorMessage] = useState('');
  const [notification, setNotification] = useState('');

  // Data lists
  const [videos, setVideos] = useState<VideoItem[]>([]);
  const [watchLaterList, setWatchLaterList] = useState<VideoItem[]>([]);
  const [historyList, setHistoryList] = useState<VideoItem[]>([]);
  const [likedList, setLikedList] = useState<string[]>([]);

  // Selected / Playing video
  const [selectedVideo, setSelectedVideo] = useState<VideoItem | null>(null);
  const [videoDetail, setVideoDetail] = useState<{ description: string; availableQualities: string[] } | null>(null);
  const [isPlaying, setIsPlaying] = useState(false);
  const [currentTime, setCurrentTime] = useState(0);
  const [volume, setVolume] = useState(80);
  const [isMuted, setIsMuted] = useState(false);
  const [speed, setSpeed] = useState(1.0);
  const [quality, setQuality] = useState('1080p');
  const [isFullscreen, setIsFullscreen] = useState(false);

  // Auth state
  const [auth, setAuth] = useState<UserAuth>({
    isLoggedIn: false,
    userName: '',
    email: '',
    avatarUrl: ''
  });

  // Settings
  const [cookiesBrowser, setCookiesBrowser] = useState('chrome');
  const [hwdecSetting, setHwdecSetting] = useState('auto-safe');

  const playerContainerRef = useRef<HTMLDivElement>(null);
  const searchInputRef = useRef<HTMLInputElement>(null);

  const showNotification = (msg: string) => {
    setNotification(msg);
    setTimeout(() => setNotification(''), 3000);
  };

  const deduplicateVideos = (list: VideoItem[]): VideoItem[] => {
    const seen = new Set<string>();
    return list.filter(item => {
      if (!item || !item.id || seen.has(item.id)) return false;
      seen.add(item.id);
      return true;
    });
  };

  // Load Initial Feed
  const fetchFeed = useCallback(async () => {
    setIsLoading(true);
    setErrorMessage('');
    try {
      const res = await fetch('/api/feed');
      if (!res.ok) throw new Error(`Feed request failed (${res.status})`);
      const data = await res.json();
      if (Array.isArray(data.videos)) {
        setVideos(deduplicateVideos(data.videos));
      }
    } catch (err: any) {
      console.error(err);
      setErrorMessage('Unable to load YouTube home feed. Check network connection or search for a video.');
    } finally {
      setIsLoading(false);
    }
  }, []);

  // Load Watch Later
  const fetchWatchLater = async () => {
    try {
      const res = await fetch('/api/watch_later');
      if (res.ok) {
        const data = await res.json();
        setWatchLaterList(deduplicateVideos(data.watchLater || []));
      }
    } catch {}
  };

  // Load History
  const fetchHistory = async () => {
    try {
      const res = await fetch('/api/history');
      if (res.ok) {
        const data = await res.json();
        setHistoryList(deduplicateVideos(data.history || []));
      }
    } catch {}
  };

  // Load Auth Status
  const fetchAuthStatus = async () => {
    try {
      const res = await fetch('/api/auth/status');
      if (res.ok) {
        const data = await res.json();
        setAuth(data);
      }
    } catch {}
  };

  useEffect(() => {
    fetchFeed();
    fetchWatchLater();
    fetchHistory();
    fetchAuthStatus();
  }, [fetchFeed]);

  // Search handler
  const handleSearch = async (queryToSearch: string) => {
    const q = queryToSearch.trim();
    if (!q) {
      fetchFeed();
      return;
    }

    setIsLoading(true);
    setErrorMessage('');
    setCurrentPage('home');

    try {
      const res = await fetch(`/api/search?q=${encodeURIComponent(q)}`);
      if (!res.ok) throw new Error(`Search failed (${res.status})`);
      const data = await res.json();
      if (Array.isArray(data.videos) && data.videos.length > 0) {
        setVideos(deduplicateVideos(data.videos));
      } else {
        setVideos([]);
      }
    } catch (err: any) {
      console.error(err);
      setErrorMessage(`Search error: ${err.message || 'Failed to fetch search results'}`);
    } finally {
      setIsLoading(false);
    }
  };

  // Select video for playback
  const handleSelectVideo = async (video: VideoItem) => {
    setSelectedVideo(video);
    setIsPlaying(true);
    setCurrentTime(0);
    setCurrentPage('watch');

    // Record to history backend
    try {
      await fetch('/api/history', {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify(video),
      });
      fetchHistory();
    } catch {}

    // Load extra metadata
    try {
      const res = await fetch(`/api/video/${video.id}`);
      if (res.ok) {
        const data = await res.json();
        setVideoDetail({
          description: data.description || '',
          availableQualities: data.availableQualities || ['1080p', '720p', '480p', '360p'],
        });
      }
    } catch {}
  };

  // Toggle watch later
  const handleToggleWatchLater = async (video: VideoItem, e?: React.MouseEvent) => {
    e?.stopPropagation();
    try {
      const res = await fetch('/api/watch_later', {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify(video),
      });
      if (res.ok) {
        const data = await res.json();
        showNotification(data.saved ? 'Saved to Watch Later' : 'Removed from Watch Later');
        fetchWatchLater();
      }
    } catch {
      showNotification('Could not update Watch Later');
    }
  };

  // Clear history
  const handleClearHistory = async () => {
    try {
      const res = await fetch('/api/history', { method: 'DELETE' });
      if (res.ok) {
        setHistoryList([]);
        showNotification('Watch history cleared');
      }
    } catch {
      showNotification('Could not clear history');
    }
  };

  // Google Login / Logout
  const handleLogin = async () => {
    try {
      const res = await fetch('/api/auth/login', {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({}),
      });
      if (res.ok) {
        const data = await res.json();
        setAuth(data.user);
        showNotification('Signed in with Google');
      }
    } catch {
      showNotification('Sign in failed');
    }
  };

  const handleLogout = async () => {
    try {
      const res = await fetch('/api/auth/logout', { method: 'POST' });
      if (res.ok) {
        setAuth({
          isLoggedIn: false,
          userName: '',
          email: '',
          avatarUrl: ''
        });
        showNotification('Signed out and cleared credentials');
      }
    } catch {
      showNotification('Sign out error');
    }
  };

  // Keyboard controls listener (Space, J, K, L, F, M, Left/Right)
  useEffect(() => {
    const handleKeyDown = (e: KeyboardEvent) => {
      if ((e.target as HTMLElement)?.tagName === 'INPUT') return;

      if (e.code === 'Space' || e.key === 'k' || e.key === 'K') {
        e.preventDefault();
        setIsPlaying(p => !p);
      } else if (e.key === 'j' || e.key === 'J') {
        setCurrentTime(t => Math.max(0, t - 10));
      } else if (e.key === 'l' || e.key === 'L') {
        if (selectedVideo) {
          setCurrentTime(t => Math.min(selectedVideo.durationSeconds, t + 10));
        }
      } else if (e.key === 'f' || e.key === 'F') {
        setIsFullscreen(f => !f);
      } else if (e.key === 'm' || e.key === 'M') {
        setIsMuted(m => !m);
      } else if (e.key === 'ArrowLeft') {
        setCurrentTime(t => Math.max(0, t - 5));
      } else if (e.key === 'ArrowRight') {
        if (selectedVideo) {
          setCurrentTime(t => Math.min(selectedVideo.durationSeconds, t + 5));
        }
      }
    };
    window.addEventListener('keydown', handleKeyDown);
    return () => window.removeEventListener('keydown', handleKeyDown);
  }, [selectedVideo]);

  // Video progress simulation ticker
  useEffect(() => {
    let interval: any;
    if (isPlaying && selectedVideo) {
      interval = setInterval(() => {
        setCurrentTime(prev => {
          if (prev >= selectedVideo.durationSeconds) {
            setIsPlaying(false);
            return 0;
          }
          return prev + 1;
        });
      }, 1000 / speed);
    }
    return () => clearInterval(interval);
  }, [isPlaying, selectedVideo, speed]);

  const formatSec = (total: number) => {
    const m = Math.floor(total / 60);
    const s = Math.floor(total % 60);
    return `${m}:${s < 10 ? '0' : ''}${s}`;
  };

  // Compute active list based on page
  const currentVideoList = currentPage === 'watch_later'
    ? watchLaterList
    : currentPage === 'history'
    ? historyList
    : videos;

  return (
    <div className="flex h-screen w-screen overflow-hidden bg-[#0e0f14] text-[#e0e2ec] font-sans select-none">
      {/* Toast Notification */}
      {notification && (
        <div className="fixed top-4 right-6 z-50 flex items-center gap-2 bg-[#1b1e2c] border border-[#2e344e] text-[#8ab4f8] px-4 py-2.5 rounded-lg shadow-xl text-xs font-medium animate-fade-in">
          <CheckCircle2 className="w-4 h-4 text-emerald-400 shrink-0" />
          {notification}
        </div>
      )}

      {/* Main Layout Container */}
      <div className="flex flex-col flex-1 h-full overflow-hidden">
        {/* Top Header Bar */}
        <header className="h-14 bg-[#121319] border-b border-[#1f212c] px-4 flex items-center justify-between gap-4 shrink-0">
          {/* Logo Brand */}
          <div className="flex items-center gap-2.5 cursor-pointer" onClick={() => { setCurrentPage('home'); fetchFeed(); }}>
            <div className="flex items-center justify-center w-7 h-7 rounded-md bg-[#ff0033] shadow-md shadow-red-950/40">
              <Play className="w-3.5 h-3.5 text-white fill-white ml-0.5" />
            </div>
            <span className="font-bold text-white text-base tracking-tight">RustTube</span>
          </div>

          {/* Centered Search Bar with Direct URL Support */}
          <div className="flex-1 max-w-xl">
            <form
              onSubmit={e => {
                e.preventDefault();
                handleSearch(searchQuery);
              }}
              className="relative w-full"
            >
              <Search className="w-4 h-4 text-[#666a7e] absolute left-3.5 top-1/2 -translate-y-1/2" />
              <input
                ref={searchInputRef}
                type="text"
                value={searchQuery}
                onChange={e => setSearchQuery(e.target.value)}
                placeholder="Search videos or paste YouTube link..."
                className="w-full bg-[#181922] border border-[#262836] rounded-lg pl-10 pr-9 py-2 text-xs text-white placeholder-[#666a7e] focus:outline-none focus:border-[#ff0033]/70 focus:ring-1 focus:ring-[#ff0033]/70 transition-all font-sans"
              />
              {searchQuery && (
                <button
                  type="button"
                  onClick={() => { setSearchQuery(''); fetchFeed(); }}
                  className="absolute right-3 top-1/2 -translate-y-1/2 text-xs text-[#82869a] hover:text-white"
                >
                  <X className="w-3.5 h-3.5" />
                </button>
              )}
            </form>
          </div>

          {/* Account Indicator */}
          <div className="flex items-center gap-3">
            {auth.isLoggedIn ? (
              <div className="flex items-center gap-2 cursor-pointer" onClick={() => setCurrentPage('settings')}>
                <div className="w-7 h-7 rounded-full bg-[#202334] border border-[#30344c] overflow-hidden flex items-center justify-center">
                  {auth.avatarUrl ? (
                    <img src={auth.avatarUrl} alt={auth.userName} className="w-full h-full object-cover" />
                  ) : (
                    <UserCheck className="w-3.5 h-3.5 text-emerald-400" />
                  )}
                </div>
                <span className="text-xs font-medium text-white max-w-[120px] truncate hidden sm:block">
                  {auth.userName}
                </span>
              </div>
            ) : (
              <button
                onClick={handleLogin}
                className="px-3 py-1.5 bg-[#ff0033] hover:bg-[#d9002c] text-white text-xs font-semibold rounded-md shadow-sm transition-colors"
              >
                Sign In
              </button>
            )}
          </div>
        </header>

        {/* Content Body */}
        <div className="flex flex-1 overflow-hidden">
          {/* Navigation Sidebar */}
          <aside className="w-52 bg-[#101117] border-r border-[#1c1e27] p-3 flex flex-col justify-between shrink-0">
            <div className="space-y-1">
              <button
                onClick={() => { setCurrentPage('home'); if (videos.length === 0) fetchFeed(); }}
                className={`w-full flex items-center gap-3 px-3 py-2 rounded-lg text-xs font-medium transition-colors ${
                  currentPage === 'home'
                    ? 'bg-[#ff0033]/15 text-[#ff4d6d] font-semibold border border-[#ff0033]/30'
                    : 'text-[#9fa3b8] hover:bg-[#161720] hover:text-white'
                }`}
              >
                <Tv className="w-4 h-4" />
                Home
              </button>
              <button
                onClick={() => setCurrentPage('subscriptions')}
                className={`w-full flex items-center gap-3 px-3 py-2 rounded-lg text-xs font-medium transition-colors ${
                  currentPage === 'subscriptions'
                    ? 'bg-[#ff0033]/15 text-[#ff4d6d] font-semibold border border-[#ff0033]/30'
                    : 'text-[#9fa3b8] hover:bg-[#161720] hover:text-white'
                }`}
              >
                <ListVideo className="w-4 h-4" />
                Subscriptions
              </button>
              <button
                onClick={() => setCurrentPage('playlists')}
                className={`w-full flex items-center gap-3 px-3 py-2 rounded-lg text-xs font-medium transition-colors ${
                  currentPage === 'playlists'
                    ? 'bg-[#ff0033]/15 text-[#ff4d6d] font-semibold border border-[#ff0033]/30'
                    : 'text-[#9fa3b8] hover:bg-[#161720] hover:text-white'
                }`}
              >
                <FolderTree className="w-4 h-4" />
                Playlists
              </button>
              <button
                onClick={() => { setCurrentPage('watch_later'); fetchWatchLater(); }}
                className={`w-full flex items-center justify-between px-3 py-2 rounded-lg text-xs font-medium transition-colors ${
                  currentPage === 'watch_later'
                    ? 'bg-[#ff0033]/15 text-[#ff4d6d] font-semibold border border-[#ff0033]/30'
                    : 'text-[#9fa3b8] hover:bg-[#161720] hover:text-white'
                }`}
              >
                <span className="flex items-center gap-3">
                  <Clock className="w-4 h-4" />
                  Watch Later
                </span>
                {watchLaterList.length > 0 && (
                  <span className="text-[10px] font-mono bg-[#1f2231] px-1.5 py-0.5 rounded text-white">
                    {watchLaterList.length}
                  </span>
                )}
              </button>
              <button
                onClick={() => { setCurrentPage('history'); fetchHistory(); }}
                className={`w-full flex items-center justify-between px-3 py-2 rounded-lg text-xs font-medium transition-colors ${
                  currentPage === 'history'
                    ? 'bg-[#ff0033]/15 text-[#ff4d6d] font-semibold border border-[#ff0033]/30'
                    : 'text-[#9fa3b8] hover:bg-[#161720] hover:text-white'
                }`}
              >
                <span className="flex items-center gap-3">
                  <History className="w-4 h-4" />
                  History
                </span>
                {historyList.length > 0 && (
                  <span className="text-[10px] font-mono bg-[#1f2231] px-1.5 py-0.5 rounded text-white">
                    {historyList.length}
                  </span>
                )}
              </button>
            </div>

            {/* Bottom Settings Button */}
            <div className="pt-3 border-t border-[#1c1e27]">
              <button
                onClick={() => setCurrentPage('settings')}
                className={`w-full flex items-center gap-3 px-3 py-2 rounded-lg text-xs font-medium transition-colors ${
                  currentPage === 'settings'
                    ? 'bg-[#ff0033]/15 text-[#ff4d6d] font-semibold border border-[#ff0033]/30'
                    : 'text-[#9fa3b8] hover:bg-[#161720] hover:text-white'
                }`}
              >
                <Settings className="w-4 h-4" />
                Settings
              </button>
            </div>
          </aside>

          {/* Main Content Workspace */}
          <main className="flex-1 flex flex-col overflow-hidden bg-[#0a0b0e]">
            {/* Scrollable Area */}
            <div className="flex-1 overflow-y-auto p-6">
              {/* Error Banner */}
              {errorMessage && (
                <div className="mb-4 p-3 bg-red-950/40 border border-red-800/50 rounded-lg flex items-center gap-3 text-xs text-red-200">
                  <AlertCircle className="w-4 h-4 shrink-0 text-red-400" />
                  <span>{errorMessage}</span>
                </div>
              )}

              {/* Dedicated Watch Page */}
              {currentPage === 'watch' && selectedVideo ? (
                <div className="max-w-5xl mx-auto space-y-5">
                  {/* Top Bar with Back Button */}
                  <div className="flex items-center justify-between">
                    <button
                      onClick={() => setCurrentPage('home')}
                      className="flex items-center gap-2 px-3 py-1.5 rounded-lg bg-[#141620] hover:bg-[#1a1d2c] border border-[#232636] text-xs text-[#a0a4b8] hover:text-white transition-colors"
                    >
                      <span>← Back to Browse</span>
                    </button>
                    <span className="text-xs text-[#71768d] font-mono">
                      {quality} • {speed}x • {hwdecSetting}
                    </span>
                  </div>

                  {/* Native / Embedded Video Player Canvas */}
                  <div className="relative aspect-video w-full rounded-2xl overflow-hidden bg-black shadow-2xl border border-[#1f212d] ring-1 ring-white/5">
                    <iframe
                      src={`${selectedVideo.embedUrl}?autoplay=1`}
                      title={selectedVideo.title}
                      className="w-full h-full object-cover"
                      allow="accelerometer; autoplay; clipboard-write; encrypted-media; gyroscope; picture-in-picture"
                      allowFullScreen
                    />
                  </div>

                  {/* Video Metadata Header */}
                  <div className="space-y-3 pt-2">
                    <h1 className="text-xl font-bold text-white leading-snug">
                      {selectedVideo.title}
                    </h1>

                    <div className="flex flex-wrap items-center justify-between gap-4 pb-2 border-b border-[#1c1e29]">
                      <div className="flex items-center gap-3">
                        <div className="w-10 h-10 rounded-full bg-gradient-to-tr from-red-600 to-red-400 flex items-center justify-center font-bold text-white text-sm shrink-0 shadow">
                          {selectedVideo.channel.slice(0, 1).toUpperCase()}
                        </div>
                        <div>
                          <p className="text-sm font-semibold text-white">{selectedVideo.channel}</p>
                          <p className="text-[11px] text-[#7b8094]">{selectedVideo.views} • {selectedVideo.published || 'Recent'}</p>
                        </div>
                      </div>

                      <div className="flex items-center gap-2">
                        {/* Like */}
                        <button
                          onClick={() => {
                            setLikedList(prev =>
                              prev.includes(selectedVideo.id)
                                ? prev.filter(id => id !== selectedVideo.id)
                                : [...prev, selectedVideo.id]
                            );
                            showNotification(likedList.includes(selectedVideo.id) ? 'Removed like' : 'Liked video');
                          }}
                          className={`flex items-center gap-1.5 px-3.5 py-1.5 rounded-lg text-xs font-medium border transition-colors ${
                            likedList.includes(selectedVideo.id)
                              ? 'bg-[#ff0033]/20 border-[#ff0033]/50 text-[#ff4d6d]'
                              : 'bg-[#151620] border-[#222533] text-[#a0a4b8] hover:text-white hover:bg-[#1c1e2b]'
                          }`}
                        >
                          <ThumbsUp className="w-3.5 h-3.5" />
                          <span>{likedList.includes(selectedVideo.id) ? 'Liked' : 'Like'}</span>
                        </button>

                        {/* Watch Later */}
                        <button
                          onClick={e => handleToggleWatchLater(selectedVideo, e)}
                          className={`flex items-center gap-1.5 px-3.5 py-1.5 rounded-lg text-xs font-medium border transition-colors ${
                            watchLaterList.some(v => v.id === selectedVideo.id)
                              ? 'bg-[#ff0033]/20 border-[#ff0033]/50 text-[#ff4d6d]'
                              : 'bg-[#151620] border-[#222533] text-[#a0a4b8] hover:text-white hover:bg-[#1c1e2b]'
                          }`}
                        >
                          {watchLaterList.some(v => v.id === selectedVideo.id) ? (
                            <>
                              <BookmarkCheck className="w-3.5 h-3.5" />
                              <span>Saved</span>
                            </>
                          ) : (
                            <>
                              <Bookmark className="w-3.5 h-3.5" />
                              <span>Watch Later</span>
                            </>
                          )}
                        </button>
                      </div>
                    </div>

                    {/* Description Box */}
                    <div className="p-4 rounded-xl bg-[#12141c] border border-[#1e202d] text-xs text-[#b0b4c8] leading-relaxed whitespace-pre-line max-h-48 overflow-y-auto">
                      {videoDetail?.description || selectedVideo.description || 'No detailed description provided for this video.'}
                    </div>
                  </div>

                  {/* Related / Up Next Videos Section */}
                  <div className="pt-4 space-y-3">
                    <h3 className="text-sm font-bold text-white">More Videos</h3>
                    <div className="grid grid-cols-1 sm:grid-cols-2 md:grid-cols-3 lg:grid-cols-4 gap-4">
                      {videos
                        .filter(v => v.id !== selectedVideo.id)
                        .slice(0, 8)
                        .map((item, idx) => (
                          <div
                            key={`related-${item.id}-${idx}`}
                            onClick={() => handleSelectVideo(item)}
                            className="group cursor-pointer rounded-xl bg-[#12131a] border border-[#1d1f2b] hover:border-[#2d3144] hover:bg-[#151722] overflow-hidden flex flex-col transition-all"
                          >
                            <div className="relative aspect-video w-full bg-[#181a24] overflow-hidden">
                              <img
                                src={item.thumbnail}
                                alt={item.title}
                                className="w-full h-full object-cover group-hover:scale-105 transition-transform duration-300"
                                loading="lazy"
                              />
                              {item.duration && (
                                <div className="absolute bottom-1.5 right-1.5 bg-black/85 text-white text-[10px] font-mono px-1.5 py-0.5 rounded">
                                  {item.duration}
                                </div>
                              )}
                            </div>
                            <div className="p-2.5 flex flex-col justify-between flex-1 gap-1">
                              <h4 className="text-xs font-semibold text-white line-clamp-2 group-hover:text-[#ff4d6d] transition-colors">
                                {item.title}
                              </h4>
                              <p className="text-[11px] text-[#7d8296] truncate">{item.channel}</p>
                            </div>
                          </div>
                        ))}
                    </div>
                  </div>
                </div>
              ) : currentPage === 'settings' ? (
                <div className="max-w-2xl mx-auto space-y-6">
                  <div>
                    <h2 className="text-xl font-bold text-white">Application Settings</h2>
                    <p className="text-xs text-[#7b8094]">Manage authentication, browser cookies, and local data.</p>
                  </div>

                  {/* Google OAuth Section */}
                  <div className="p-4 rounded-xl bg-[#13141b] border border-[#20222d] space-y-3">
                    <div className="flex items-center justify-between">
                      <div>
                        <h3 className="text-sm font-semibold text-white">Google Account</h3>
                        <p className="text-xs text-[#7d8296]">Personal OAuth2 PKCE authorization</p>
                      </div>
                      {auth.isLoggedIn ? (
                        <span className="flex items-center gap-1.5 text-xs text-emerald-400 bg-emerald-500/10 px-2.5 py-1 rounded-full border border-emerald-500/20 font-medium">
                          <CheckCircle2 className="w-3.5 h-3.5" /> Connected
                        </span>
                      ) : (
                        <span className="text-xs text-[#7d8296] bg-[#1a1c26] px-2.5 py-1 rounded-full">
                          Signed Out
                        </span>
                      )}
                    </div>

                    {auth.isLoggedIn ? (
                      <div className="pt-2 flex items-center justify-between">
                        <div>
                          <p className="text-xs font-medium text-white">{auth.userName}</p>
                          <p className="text-[11px] text-[#787d92]">{auth.email}</p>
                        </div>
                        <button
                          onClick={handleLogout}
                          className="flex items-center gap-1.5 px-3 py-1.5 bg-red-950/40 hover:bg-red-900/50 text-red-400 text-xs rounded-md border border-red-800/40 font-medium transition-colors"
                        >
                          <LogOut className="w-3.5 h-3.5" /> Sign Out
                        </button>
                      </div>
                    ) : (
                      <div className="pt-2">
                        <button
                          onClick={handleLogin}
                          className="px-4 py-2 bg-[#ff0033] hover:bg-[#d9002c] text-white text-xs font-semibold rounded-md shadow transition-colors"
                        >
                          Sign In with Google
                        </button>
                      </div>
                    )}
                  </div>

                  {/* Browser Cookies Selector */}
                  <div className="p-4 rounded-xl bg-[#13141b] border border-[#20222d] space-y-3">
                    <h3 className="text-sm font-semibold text-white">Authenticated Playback</h3>
                    <p className="text-xs text-[#7d8296]">Use browser session cookies with yt-dlp to access age-restricted or premium content.</p>
                    <div className="max-w-xs">
                      <select
                        value={cookiesBrowser}
                        onChange={e => setCookiesBrowser(e.target.value)}
                        className="w-full bg-[#181922] border border-[#272938] rounded px-3 py-2 text-xs text-white focus:outline-none"
                      >
                        <option value="chrome">Google Chrome</option>
                        <option value="edge">Microsoft Edge</option>
                        <option value="firefox">Mozilla Firefox</option>
                        <option value="none">None (Public Streams)</option>
                      </select>
                    </div>
                  </div>

                  {/* Local Storage Section */}
                  <div className="p-4 rounded-xl bg-[#13141b] border border-[#20222d] space-y-3">
                    <h3 className="text-sm font-semibold text-white">Local Storage</h3>
                    <p className="text-xs text-[#7d8296]">Manage locally stored watch history and cached records.</p>
                    <div className="flex gap-3 pt-1">
                      <button
                        onClick={handleClearHistory}
                        className="px-3 py-1.5 bg-[#181a24] hover:bg-[#202332] text-xs text-[#ff6b81] border border-[#292c3e] rounded-md transition-colors"
                      >
                        Clear Watch History ({historyList.length})
                      </button>
                      <button
                        onClick={() => showNotification('Cache cleaned')}
                        className="px-3 py-1.5 bg-[#181a24] hover:bg-[#202332] text-xs text-[#9ea3b8] border border-[#292c3e] rounded-md transition-colors"
                      >
                        Clear Thumbnail Cache
                      </button>
                    </div>
                  </div>
                </div>
              ) : currentPage === 'subscriptions' ? (
                /* Subscriptions View */
                <div className="space-y-4">
                  <h2 className="text-base font-bold text-white">Subscribed Channels</h2>
                  {auth.isLoggedIn ? (
                    <div className="grid grid-cols-1 md:grid-cols-2 lg:grid-cols-3 gap-4">
                      {videos.slice(0, 6).map((v, idx) => (
                        <div key={idx} className="p-4 rounded-xl bg-[#13141b] border border-[#1e202b] flex items-center gap-3">
                          <div className="w-10 h-10 rounded-full bg-[#202332] overflow-hidden shrink-0">
                            <img src={v.thumbnail} alt="" className="w-full h-full object-cover" />
                          </div>
                          <div className="truncate">
                            <p className="text-xs font-semibold text-white truncate">{v.channel}</p>
                            <p className="text-[11px] text-[#7d8296]">Subscribed</p>
                          </div>
                        </div>
                      ))}
                    </div>
                  ) : (
                    <div className="py-20 text-center space-y-3">
                      <ListVideo className="w-8 h-8 text-[#54586d] mx-auto" />
                      <p className="text-sm text-[#8c91a6]">Sign in to view your subscribed channels.</p>
                      <button
                        onClick={handleLogin}
                        className="px-4 py-2 bg-[#ff0033] hover:bg-[#d9002c] text-white text-xs font-semibold rounded-md shadow transition-colors"
                      >
                        Sign In with Google
                      </button>
                    </div>
                  )}
                </div>
              ) : currentPage === 'playlists' ? (
                /* Playlists View */
                <div className="space-y-4">
                  <h2 className="text-base font-bold text-white">Your Playlists</h2>
                  {auth.isLoggedIn ? (
                    <div className="grid grid-cols-1 md:grid-cols-3 gap-4">
                      <div className="p-4 rounded-xl bg-[#13141b] border border-[#1e202b] space-y-2">
                        <div className="aspect-video bg-[#1f2231] rounded-lg overflow-hidden flex items-center justify-center">
                          <FolderTree className="w-8 h-8 text-[#7d8296]" />
                        </div>
                        <p className="text-xs font-semibold text-white">Liked Videos</p>
                        <p className="text-[11px] text-[#7d8296]">{likedList.length} videos</p>
                      </div>
                    </div>
                  ) : (
                    <div className="py-20 text-center space-y-3">
                      <FolderTree className="w-8 h-8 text-[#54586d] mx-auto" />
                      <p className="text-sm text-[#8c91a6]">Sign in to access your YouTube playlists.</p>
                      <button
                        onClick={handleLogin}
                        className="px-4 py-2 bg-[#ff0033] hover:bg-[#d9002c] text-white text-xs font-semibold rounded-md shadow transition-colors"
                      >
                        Sign In with Google
                      </button>
                    </div>
                  )}
                </div>
              ) : (
                /* Video Grid (Home / Watch Later / History) */
                <div className="space-y-4">
                  <div className="flex items-center justify-between">
                    <div>
                      <h2 className="text-base font-bold text-white">
                        {currentPage === 'home' && (searchQuery ? `Search results for "${searchQuery}"` : 'Home Feed')}
                        {currentPage === 'watch_later' && 'Watch Later'}
                        {currentPage === 'history' && 'Watch History'}
                      </h2>
                      <p className="text-xs text-[#7b8094]">
                        {currentVideoList.length} {currentVideoList.length === 1 ? 'video' : 'videos'}
                      </p>
                    </div>

                    {currentPage === 'history' && historyList.length > 0 && (
                      <button
                        onClick={handleClearHistory}
                        className="text-xs text-[#ff4d6d] hover:underline"
                      >
                        Clear History
                      </button>
                    )}
                  </div>

                  {/* Loading State */}
                  {isLoading ? (
                    <div className="py-24 flex flex-col items-center justify-center gap-3">
                      <Loader2 className="w-7 h-7 text-[#ff0033] animate-spin" />
                      <p className="text-xs text-[#8c91a6]">Loading videos from YouTube...</p>
                    </div>
                  ) : currentVideoList.length === 0 ? (
                    /* Empty State */
                    <div className="py-24 text-center space-y-2">
                      <AlertCircle className="w-8 h-8 text-[#4a4d60] mx-auto" />
                      <p className="text-sm text-[#8c91a6]">
                        {currentPage === 'watch_later'
                          ? 'No videos in Watch Later. Save videos using the bookmark icon.'
                          : currentPage === 'history'
                          ? 'No watch history yet.'
                          : 'No videos found.'}
                      </p>
                      {currentPage !== 'home' && (
                        <button
                          onClick={() => { setCurrentPage('home'); fetchFeed(); }}
                          className="text-xs text-[#ff4d6d] hover:underline pt-1 inline-block"
                        >
                          Explore Home Feed
                        </button>
                      )}
                    </div>
                  ) : (
                    /* Real Video Card Grid */
                    <div className="grid grid-cols-1 md:grid-cols-2 lg:grid-cols-3 xl:grid-cols-4 gap-4">
                      {currentVideoList.map((video, idx) => {
                        const isSelected = selectedVideo?.id === video.id;
                        const inWatchLater = watchLaterList.some(v => v.id === video.id);

                        return (
                          <div
                            key={`grid-${video.id}-${idx}`}
                            onClick={() => handleSelectVideo(video)}
                            className={`group cursor-pointer rounded-xl bg-[#12131a] border transition-all duration-200 overflow-hidden flex flex-col ${
                              isSelected
                                ? 'border-[#ff0033]/70 ring-1 ring-[#ff0033]/70 shadow-lg shadow-red-950/20'
                                : 'border-[#1d1f2b] hover:border-[#2d3144] hover:bg-[#151722]'
                            }`}
                          >
                            {/* Thumbnail */}
                            <div className="relative aspect-video w-full bg-[#181a24] overflow-hidden">
                              <img
                                src={video.thumbnail}
                                alt={video.title}
                                className="w-full h-full object-cover group-hover:scale-105 transition-transform duration-300"
                                loading="lazy"
                              />

                              {/* Duration Pill */}
                              {video.duration && (
                                <div className="absolute bottom-2 right-2 bg-black/85 backdrop-blur-sm text-white text-[11px] font-mono px-1.5 py-0.5 rounded font-medium">
                                  {video.duration}
                                </div>
                              )}

                              {/* Watch Later Quick Action */}
                              <button
                                onClick={e => handleToggleWatchLater(video, e)}
                                title={inWatchLater ? 'Remove from Watch Later' : 'Save to Watch Later'}
                                className="absolute top-2 right-2 p-1.5 rounded-md bg-black/75 hover:bg-black/90 text-white opacity-0 group-hover:opacity-100 transition-opacity"
                              >
                                {inWatchLater ? (
                                  <BookmarkCheck className="w-4 h-4 text-[#ff4d6d]" />
                                ) : (
                                  <Bookmark className="w-4 h-4" />
                                )}
                              </button>

                              {isSelected && isPlaying && (
                                <div className="absolute top-2 left-2 flex items-center gap-1.5 bg-[#ff0033] text-white text-[10px] font-semibold px-2 py-0.5 rounded-full shadow">
                                  <Play className="w-2.5 h-2.5 fill-white" /> PLAYING
                                </div>
                              )}
                            </div>

                            {/* Metadata */}
                            <div className="p-3 flex flex-col justify-between flex-1 gap-1.5">
                              <h3 className="text-xs font-semibold text-white line-clamp-2 leading-relaxed group-hover:text-[#ff4d6d] transition-colors">
                                {video.title}
                              </h3>

                              <div className="flex items-center justify-between text-[11px] text-[#7d8296] pt-1">
                                <span className="truncate max-w-[140px] text-[#9ea3b8]">
                                  {video.channel}
                                </span>
                                <span>{video.views}</span>
                              </div>
                            </div>
                          </div>
                        );
                      })}
                    </div>
                  )}
                </div>
              )}
            </div>

            {/* Docked Mini-Player Bar when browsing other views */}
            {selectedVideo && currentPage !== 'watch' && (
              <div ref={playerContainerRef} className="border-t border-[#1e202b] bg-[#111218] px-5 py-2.5 shrink-0 shadow-2xl flex items-center justify-between gap-4">
                {/* Left: Thumbnail & Info */}
                <div 
                  onClick={() => setCurrentPage('watch')} 
                  className="flex items-center gap-3 cursor-pointer group max-w-sm truncate"
                >
                  <div className="relative w-14 h-9 rounded bg-[#1a1c26] overflow-hidden shrink-0 border border-[#272a3b]">
                    <img src={selectedVideo.thumbnail} alt="" className="w-full h-full object-cover" />
                    <div className="absolute inset-0 bg-black/30 group-hover:bg-black/10 flex items-center justify-center">
                      <Play className="w-3.5 h-3.5 fill-white text-white opacity-80 group-hover:opacity-100" />
                    </div>
                  </div>
                  <div className="truncate">
                    <p className="text-xs font-semibold text-white truncate group-hover:text-[#ff4d6d] transition-colors">{selectedVideo.title}</p>
                    <p className="text-[11px] text-[#7d8296] truncate">{selectedVideo.channel}</p>
                  </div>
                </div>


                {/* Right: Expand and Close */}
                <div className="flex items-center gap-2">
                  <button
                    onClick={() => setCurrentPage('watch')}
                    className="flex items-center gap-1.5 px-3 py-1.5 bg-[#1a1c26] hover:bg-[#222533] text-xs text-white rounded-md border border-[#2b2e40] font-medium transition-colors"
                  >
                    <Maximize2 className="w-3.5 h-3.5" />
                    <span>Watch View</span>
                  </button>
                  <button
                    onClick={() => setSelectedVideo(null)}
                    title="Close player"
                    className="p-1.5 text-[#7b8094] hover:text-white hover:bg-[#1a1c26] rounded-md transition-colors"
                  >
                    <X className="w-4 h-4" />
                  </button>
                </div>
              </div>
            )}
          </main>
        </div>
      </div>
    </div>
  );
}
