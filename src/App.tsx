/**
 * @license
 * SPDX-License-Identifier: Apache-2.0
 */

import React, { useState, useEffect, useRef, useCallback } from 'react';
import {
  Play,
  Pause,
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
  ExternalLink,
  RefreshCw,
  ArrowLeft,
  Plus,
  Trash2,
  Edit3,
  ShieldCheck,
  Download,
  ChevronRight,
  Lock,
  Globe,
  EyeOff,
  ArrowUp,
  ArrowDown,
  SkipForward
} from 'lucide-react';

type PageId =
  | 'home'
  | 'subscriptions'
  | 'channel'
  | 'playlists'
  | 'playlist_detail'
  | 'liked'
  | 'watch_later'
  | 'history'
  | 'watch'
  | 'search'
  | 'settings';

type FeedCategory =
  | 'personalized'
  | 'subscriptions'
  | 'liked_related'
  | 'continue_watching'
  | 'trending'
  | 'experimental_cookie';

interface VideoItem {
  id: string;
  title: string;
  channel: string;
  channelId: string;
  subscribers?: string;
  duration: string;
  durationSeconds: number;
  views: string;
  viewCount?: number;
  published: string;
  publishedTimestamp?: number;
  thumbnail: string;
  embedUrl: string;
  description?: string;
  sourceTag?: string;
  watchedAt?: number;
  lastPositionSeconds?: number;
  availability?: 'Available' | 'Private' | 'Deleted' | 'Unavailable';
}

interface ChannelItem {
  channelId: string;
  subscriptionId: string;
  title: string;
  handle: string;
  description: string;
  avatarUrl: string;
  subscriberCount: string;
  videoCount: number;
  uploadsPlaylistId: string;
  channelUrl: string;
  syncedAt: number;
}

interface PlaylistVideoEntry {
  playlistItemId: string;
  playlistId: string;
  position: number;
  availability: 'Available' | 'Private' | 'Deleted' | 'Unavailable';
  addedAt: number;
  video: VideoItem;
}

interface PlaylistItem {
  playlistId: string;
  title: string;
  description: string;
  privacyStatus: 'public' | 'unlisted' | 'private';
  itemCount: number;
  thumbnailUrl: string;
  isOwned: boolean;
  updatedAt: number;
  items?: PlaylistVideoEntry[];
}

interface ClassifiedError {
  kind:
    | 'Authentication'
    | 'AuthorizationScope'
    | 'Network'
    | 'QuotaExceeded'
    | 'Parsing'
    | 'Database';
  message: string;
  occurredAt: number;
}

interface UserAuth {
  isLoggedIn: boolean;
  userName: string;
  email: string;
  handle: string;
  channelId: string;
  avatarUrl: string;
  refreshTokenStoredInKeyring: boolean;
  tokenExpiresAt?: number;
}

interface AppSettings {
  cookiesBrowser: 'none' | 'chrome' | 'edge' | 'firefox';
  hwdec: 'auto-safe' | 'd3d11va' | 'nvdec' | 'software';
  autoSyncEnabled: boolean;
  syncIntervalMinutes: number;
  experimentalCookieHomeFeed: boolean;
  experimentalCookieWatchLater: boolean;
  autoCheckUpdates: boolean;
  includePrereleases: boolean;
  resumePlayback: boolean;
}

interface SyncState {
  lastSuccessfulSync: number | null;
  lastSyncAttempt: number | null;
  lastError: ClassifiedError | null;
  subscriptionsCount: number;
  playlistsCount: number;
  likedCount: number;
}

interface UpdaterState {
  currentVersion: string;
  latestRelease: {
    version: string;
    tagName: string;
    publishedAt: string;
    releaseNotes: string;
    htmlUrl: string;
    assetName: string;
    downloadUrl: string;
    sizeBytes: number;
    sha256: string;
  } | null;
  lastCheckedAt: number | null;
  status:
    | 'idle'
    | 'checking'
    | 'up_to_date'
    | 'available'
    | 'downloading'
    | 'staged'
    | 'applied'
    | 'error';
  stagedVersion: string | null;
  stagedFilePath: string | null;
  sha256Verified: boolean;
  errorMessage: string | null;
  rolledBack: boolean;
  repository?: string;
  userDataProtected?: {
    configPath: string;
    databasePath: string;
    credentialManager: string;
    watchLaterCount: number;
    historyCount: number;
  };
}

export default function App() {
  const [currentPage, setCurrentPage] = useState<PageId>('home');
  const [navStack, setNavStack] = useState<PageId[]>([]);
  const [feedCategory, setFeedCategory] = useState<FeedCategory>('personalized');
  const [feedPage, setFeedPage] = useState(1);
  const [feedHasMore, setFeedHasMore] = useState(false);

  const [searchQuery, setSearchQuery] = useState('');
  const [activeSearchTerm, setActiveSearchTerm] = useState('');
  const [subscriptionsFilter, setSubscriptionsFilter] = useState('');
  const [historySearch, setHistorySearch] = useState('');

  const [isLoading, setIsLoading] = useState(false);
  const [isSyncing, setIsSyncing] = useState(false);
  const [isMutating, setIsMutating] = useState(false);
  const [errorMessage, setErrorMessage] = useState('');
  const [classifiedError, setClassifiedError] = useState<ClassifiedError | null>(null);
  const [notification, setNotification] = useState('');

  // Data collections
  const [homeVideos, setHomeVideos] = useState<VideoItem[]>([]);
  const [searchVideos, setSearchVideos] = useState<VideoItem[]>([]);
  const [subscriptions, setSubscriptions] = useState<ChannelItem[]>([]);
  const [confirmUnsubscribeId, setConfirmUnsubscribeId] = useState<string | null>(null);

  // Active Channel state
  const [activeChannel, setActiveChannel] = useState<ChannelItem | null>(null);
  const [activeChannelSubscribed, setActiveChannelSubscribed] = useState(false);
  const [channelVideos, setChannelVideos] = useState<VideoItem[]>([]);
  const [channelPage, setChannelPage] = useState(1);
  const [channelHasMore, setChannelHasMore] = useState(false);

  // Playlists state
  const [playlists, setPlaylists] = useState<PlaylistItem[]>([]);
  const [activePlaylist, setActivePlaylist] = useState<PlaylistItem | null>(null);
  const [playlistItems, setPlaylistItems] = useState<PlaylistVideoEntry[]>([]);
  const [newPlaylistTitle, setNewPlaylistTitle] = useState('');
  const [newPlaylistDesc, setNewPlaylistDesc] = useState('');
  const [newPlaylistPrivacy, setNewPlaylistPrivacy] = useState<'private' | 'unlisted' | 'public'>('private');
  const [isEditingPlaylist, setIsEditingPlaylist] = useState(false);
  const [editPlaylistTitle, setEditPlaylistTitle] = useState('');
  const [editPlaylistDesc, setEditPlaylistDesc] = useState('');
  const [editPlaylistPrivacy, setEditPlaylistPrivacy] = useState<'private' | 'unlisted' | 'public'>('private');
  const [confirmDeletePlaylistId, setConfirmDeletePlaylistId] = useState<string | null>(null);
  const [playlistDropdownVideoId, setPlaylistDropdownVideoId] = useState<string | null>(null);

  // Watch Later (Local vs Experimental YouTube Cookie)
  const [localWatchLater, setLocalWatchLater] = useState<VideoItem[]>([]);
  const [experimentalWatchLater, setExperimentalWatchLater] = useState<VideoItem[]>([]);

  // History & Liked Videos
  const [historyList, setHistoryList] = useState<VideoItem[]>([]);
  const [likedVideos, setLikedVideos] = useState<VideoItem[]>([]);

  // Playback & Queue state
  const [selectedVideo, setSelectedVideo] = useState<VideoItem | null>(null);
  const [playbackQueue, setPlaybackQueue] = useState<VideoItem[]>([]);
  const [videoDetail, setVideoDetail] = useState<{
    description: string;
    availableQualities: string[];
  } | null>(null);
  const [isPlaying, setIsPlaying] = useState(false);
  const [currentTime, setCurrentTime] = useState(0);
  const [speed, setSpeed] = useState(1.0);
  const [quality, setQuality] = useState('1080p');

  // Auth, Sync, Settings, Updater
  const [auth, setAuth] = useState<UserAuth>({
    isLoggedIn: true,
    userName: 'Duong Duc Dung Nguyen',
    email: 'hshshuy36@gmail.com',
    handle: '@duongducdungnguyen',
    channelId: 'UC_personal_rusttube_user',
    avatarUrl: '/assets/aistudio/image_9995d559.png',
    refreshTokenStoredInKeyring: true,
  });
  const [oauthAccessTokenInput, setOauthAccessTokenInput] = useState('');

  const [settings, setSettings] = useState<AppSettings>({
    cookiesBrowser: 'none',
    hwdec: 'auto-safe',
    autoSyncEnabled: true,
    syncIntervalMinutes: 30,
    experimentalCookieHomeFeed: false,
    experimentalCookieWatchLater: false,
    autoCheckUpdates: true,
    includePrereleases: false,
    resumePlayback: true,
  });

  const [syncState, setSyncState] = useState<SyncState>({
    lastSuccessfulSync: null,
    lastSyncAttempt: null,
    lastError: null,
    subscriptionsCount: 0,
    playlistsCount: 0,
    likedCount: 0,
  });

  const [updater, setUpdater] = useState<UpdaterState>({
    currentVersion: '0.2.0',
    latestRelease: null,
    lastCheckedAt: null,
    status: 'idle',
    stagedVersion: null,
    stagedFilePath: null,
    sha256Verified: false,
    errorMessage: null,
    rolledBack: false,
  });

  const searchInputRef = useRef<HTMLInputElement>(null);

  const showNotification = useCallback((msg: string) => {
    setNotification(msg);
    setTimeout(() => {
      setNotification((prev) => (prev === msg ? '' : prev));
    }, 3500);
  }, []);

  const navigateTo = useCallback(
    (target: PageId) => {
      setErrorMessage('');
      setPlaylistDropdownVideoId(null);
      setCurrentPage((prev) => {
        if (prev !== target) {
          setNavStack((stack) => [...stack.slice(-15), prev]);
        }
        return target;
      });
    },
    []
  );

  const navigateBack = useCallback(() => {
    setErrorMessage('');
    setPlaylistDropdownVideoId(null);
    setNavStack((stack) => {
      if (stack.length === 0) return stack;
      const previous = stack[stack.length - 1];
      setCurrentPage(previous);
      return stack.slice(0, -1);
    });
  }, []);

  // Load Home Feed by Category
  const fetchHomeFeed = useCallback(
    async (category: FeedCategory, page = 1, refresh = false) => {
      setIsLoading(true);
      setErrorMessage('');
      try {
        const res = await fetch(
          `/api/feed?category=${encodeURIComponent(category)}&page=${page}&refresh=${refresh}`
        );
        const data = await res.json();
        if (!res.ok) {
          setErrorMessage(data.error || `Feed error (${res.status})`);
          if (data.kind) {
            setClassifiedError({
              kind: data.kind,
              message: data.error,
              occurredAt: Date.now(),
            });
          }
          setHomeVideos([]);
          return;
        }
        if (page === 1) {
          setHomeVideos(data.videos || []);
        } else {
          setHomeVideos((prev) => {
            const seen = new Set(prev.map((v) => v.id));
            const appended = (data.videos || []).filter((v: VideoItem) => !seen.has(v.id));
            return [...prev, ...appended];
          });
        }
        setFeedPage(page);
        setFeedHasMore(Boolean(data.hasMore));
      } catch (err: any) {
        setErrorMessage(err.message || 'Network error loading feed');
      } finally {
        setIsLoading(false);
      }
    },
    []
  );

  const fetchSubscriptions = useCallback(async (query = '') => {
    try {
      const res = await fetch(`/api/subscriptions?q=${encodeURIComponent(query)}`);
      if (res.ok) {
        const data = await res.json();
        setSubscriptions(data.subscriptions || []);
      }
    } catch {}
  }, []);

  const fetchPlaylists = useCallback(async () => {
    try {
      const res = await fetch('/api/playlists');
      if (res.ok) {
        const data = await res.json();
        setPlaylists(data.playlists || []);
      }
    } catch {}
  }, []);

  const fetchLikedVideos = useCallback(async () => {
    try {
      const res = await fetch('/api/liked');
      if (res.ok) {
        const data = await res.json();
        setLikedVideos(data.likedVideos || []);
      }
    } catch {}
  }, []);

  const fetchWatchLater = useCallback(async () => {
    try {
      const res = await fetch('/api/watch_later');
      if (res.ok) {
        const data = await res.json();
        setLocalWatchLater(data.localWatchLater || data.watchLater || []);
        setExperimentalWatchLater(data.experimentalYouTubeWatchLater || []);
      }
    } catch {}
  }, []);

  const fetchHistory = useCallback(async (query = '') => {
    try {
      const res = await fetch(`/api/history?q=${encodeURIComponent(query)}`);
      if (res.ok) {
        const data = await res.json();
        setHistoryList(data.history || []);
      }
    } catch {}
  }, []);

  const fetchAuthAndSyncStatus = useCallback(async () => {
    try {
      const res = await fetch('/api/auth/status');
      if (res.ok) {
        const data = await res.json();
        setAuth({
          isLoggedIn: data.isLoggedIn,
          userName: data.userName,
          email: data.email,
          handle: data.handle,
          channelId: data.channelId,
          avatarUrl: data.avatarUrl,
          refreshTokenStoredInKeyring: data.refreshTokenStoredInKeyring,
          tokenExpiresAt: data.tokenExpiresAt,
        });
        if (data.settings) setSettings(data.settings);
        if (data.sync) setSyncState(data.sync);
      }
    } catch {}
  }, []);

  const fetchUpdaterStatus = useCallback(async (autoCheck = false) => {
    try {
      const res = await fetch('/api/updater/status');
      if (res.ok) {
        const data = await res.json();
        setUpdater(data);
        if (autoCheck && data.status === 'idle') {
          const checkRes = await fetch('/api/updater/check', { method: 'POST' });
          if (checkRes.ok) {
            const checkData = await checkRes.json();
            setUpdater(checkData.updater);
          }
        }
      }
    } catch {}
  }, []);

  // Initial Startup Lifecycle: Restore session, load cached SQLite data, check GitHub Releases
  useEffect(() => {
    fetchAuthAndSyncStatus();
    fetchHomeFeed('personalized', 1, false);
    fetchSubscriptions();
    fetchPlaylists();
    fetchLikedVideos();
    fetchWatchLater();
    fetchHistory();
    fetchUpdaterStatus(true);
  }, [
    fetchAuthAndSyncStatus,
    fetchHomeFeed,
    fetchSubscriptions,
    fetchPlaylists,
    fetchLikedVideos,
    fetchWatchLater,
    fetchHistory,
    fetchUpdaterStatus,
  ]);

  // Open Dedicated Channel Page
  const handleOpenChannel = async (channelId: string, page = 1) => {
    if (!channelId) return;
    setIsLoading(true);
    setErrorMessage('');
    if (currentPage !== 'channel') {
      navigateTo('channel');
    }
    try {
      const res = await fetch(
        `/api/channel/${encodeURIComponent(channelId)}?page=${page}`
      );
      if (!res.ok) throw new Error('Failed to load channel page');
      const data = await res.json();
      setActiveChannel(data.channel);
      setActiveChannelSubscribed(Boolean(data.isSubscribed));
      if (page === 1) {
        setChannelVideos(data.videos || []);
      } else {
        setChannelVideos((prev) => [...prev, ...(data.videos || [])]);
      }
      setChannelPage(page);
      setChannelHasMore(Boolean(data.hasMore));
    } catch (err: any) {
      setErrorMessage(err.message || 'Could not load channel details');
    } finally {
      setIsLoading(false);
    }
  };

  // Subscribe / Unsubscribe Channel
  const handleUnsubscribeChannel = async (channelId: string) => {
    if (isMutating) return;
    setIsMutating(true);
    try {
      const res = await fetch(`/api/subscriptions/${encodeURIComponent(channelId)}`, {
        method: 'DELETE',
      });
      if (!res.ok) throw new Error('Unsubscribe request failed');
      setConfirmUnsubscribeId(null);
      setActiveChannelSubscribed(false);
      await fetchSubscriptions(subscriptionsFilter);
      await fetchAuthAndSyncStatus();
      showNotification('Unsubscribed and updated local database');
    } catch (err: any) {
      setErrorMessage(err.message || 'Failed to unsubscribe');
    } finally {
      setIsMutating(false);
    }
  };

  const handleSubscribeChannel = async (channel: ChannelItem) => {
    if (isMutating) return;
    setIsMutating(true);
    try {
      const res = await fetch('/api/subscriptions', {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify(channel),
      });
      if (!res.ok) throw new Error('Subscribe request failed');
      setActiveChannelSubscribed(true);
      await fetchSubscriptions(subscriptionsFilter);
      await fetchAuthAndSyncStatus();
      showNotification(`Subscribed to ${channel.title}`);
    } catch (err: any) {
      setErrorMessage(err.message || 'Failed to subscribe');
    } finally {
      setIsMutating(false);
    }
  };

  // Open Dedicated Playlist Detail View
  const handleOpenPlaylist = async (playlistId: string) => {
    setIsLoading(true);
    setErrorMessage('');
    setIsEditingPlaylist(false);
    navigateTo('playlist_detail');
    try {
      const res = await fetch(`/api/playlists/${encodeURIComponent(playlistId)}`);
      if (!res.ok) throw new Error('Could not load playlist');
      const data = await res.json();
      setActivePlaylist(data.playlist);
      setPlaylistItems(data.items || []);
      setEditPlaylistTitle(data.playlist.title);
      setEditPlaylistDesc(data.playlist.description);
      setEditPlaylistPrivacy(data.playlist.privacyStatus);
    } catch (err: any) {
      setErrorMessage(err.message || 'Failed to load playlist');
    } finally {
      setIsLoading(false);
    }
  };

  // Create Playlist
  const handleCreatePlaylist = async (e: React.FormEvent) => {
    e.preventDefault();
    if (!newPlaylistTitle.trim() || isMutating) return;
    setIsMutating(true);
    try {
      const res = await fetch('/api/playlists', {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({
          title: newPlaylistTitle.trim(),
          description: newPlaylistDesc.trim(),
          privacyStatus: newPlaylistPrivacy,
        }),
      });
      const data = await res.json();
      if (!res.ok) throw new Error(data.error || 'Failed to create playlist');
      setNewPlaylistTitle('');
      setNewPlaylistDesc('');
      await fetchPlaylists();
      await fetchAuthAndSyncStatus();
      showNotification(`Created playlist "${data.playlist.title}"`);
    } catch (err: any) {
      setErrorMessage(err.message);
    } finally {
      setIsMutating(false);
    }
  };

  // Update Playlist Title, Description & Privacy
  const handleUpdatePlaylist = async (e: React.FormEvent) => {
    e.preventDefault();
    if (!activePlaylist || !editPlaylistTitle.trim() || isMutating) return;
    setIsMutating(true);
    try {
      const res = await fetch(
        `/api/playlists/${encodeURIComponent(activePlaylist.playlistId)}`,
        {
          method: 'PUT',
          headers: { 'Content-Type': 'application/json' },
          body: JSON.stringify({
            title: editPlaylistTitle.trim(),
            description: editPlaylistDesc.trim(),
            privacyStatus: editPlaylistPrivacy,
          }),
        }
      );
      const data = await res.json();
      if (!res.ok) throw new Error(data.error || 'Failed to update playlist');
      setActivePlaylist(data.playlist);
      setIsEditingPlaylist(false);
      await fetchPlaylists();
      showNotification('Updated playlist details and privacy settings');
    } catch (err: any) {
      setErrorMessage(err.message);
    } finally {
      setIsMutating(false);
    }
  };

  // Delete Playlist
  const handleDeletePlaylist = async (playlistId: string) => {
    if (isMutating) return;
    setIsMutating(true);
    try {
      const res = await fetch(`/api/playlists/${encodeURIComponent(playlistId)}`, {
        method: 'DELETE',
      });
      if (!res.ok) throw new Error('Failed to delete playlist');
      setConfirmDeletePlaylistId(null);
      await fetchPlaylists();
      await fetchAuthAndSyncStatus();
      if (currentPage === 'playlist_detail') {
        navigateTo('playlists');
      }
      showNotification('Playlist deleted');
    } catch (err: any) {
      setErrorMessage(err.message);
    } finally {
      setIsMutating(false);
    }
  };

  // Add / Remove / Reorder Video in Playlist
  const handleAddVideoToPlaylist = async (playlistId: string, video: VideoItem) => {
    if (isMutating) return;
    setIsMutating(true);
    try {
      const res = await fetch(`/api/playlists/${encodeURIComponent(playlistId)}/items`, {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({ video }),
      });
      const data = await res.json();
      if (!res.ok) throw new Error(data.error || 'Failed to add video to playlist');
      setPlaylistDropdownVideoId(null);
      await fetchPlaylists();
      showNotification(`Added "${video.title.slice(0, 32)}" to playlist`);
    } catch (err: any) {
      setErrorMessage(err.message);
    } finally {
      setIsMutating(false);
    }
  };

  const handleRemovePlaylistItem = async (playlistId: string, playlistItemId: string) => {
    if (isMutating) return;
    setIsMutating(true);
    try {
      const res = await fetch(
        `/api/playlists/${encodeURIComponent(playlistId)}/items/${encodeURIComponent(
          playlistItemId
        )}`,
        { method: 'DELETE' }
      );
      if (!res.ok) throw new Error('Failed to remove video from playlist');
      await handleOpenPlaylist(playlistId);
      await fetchPlaylists();
      showNotification('Removed video from playlist');
    } catch (err: any) {
      setErrorMessage(err.message);
    } finally {
      setIsMutating(false);
    }
  };

  const handleReorderPlaylistItem = async (
    playlistId: string,
    playlistItemId: string,
    direction: 'up' | 'down'
  ) => {
    try {
      const res = await fetch(
        `/api/playlists/${encodeURIComponent(playlistId)}/reorder`,
        {
          method: 'PUT',
          headers: { 'Content-Type': 'application/json' },
          body: JSON.stringify({ playlistItemId, direction }),
        }
      );
      if (res.ok) {
        const data = await res.json();
        setPlaylistItems(data.items || []);
      }
    } catch {}
  };

  // Search handler (with URL parsing redirect support for playlists/channels/direct videos)
  const handleSearch = async (queryToSearch: string) => {
    const q = queryToSearch.trim();
    if (!q) {
      navigateTo('home');
      fetchHomeFeed(feedCategory, 1, false);
      return;
    }

    setIsLoading(true);
    setErrorMessage('');
    setActiveSearchTerm(q);
    navigateTo('search');

    try {
      const res = await fetch(`/api/search?q=${encodeURIComponent(q)}`);
      if (!res.ok) throw new Error(`Search failed (${res.status})`);
      const data = await res.json();

      if (data.redirectChannelId) {
        await handleOpenChannel(data.redirectChannelId);
        return;
      }
      if (data.redirectPlaylistId) {
        await handleOpenPlaylist(data.redirectPlaylistId);
        return;
      }

      setSearchVideos(data.videos || []);
    } catch (err: any) {
      setErrorMessage(`Search error: ${err.message || 'Failed to fetch search results'}`);
    } finally {
      setIsLoading(false);
    }
  };

  // Play Video (with optional sequential playlist queue and resume timestamp)
  const handleSelectVideo = async (
    video: VideoItem,
    queue?: VideoItem[],
    resumeSeconds?: number
  ) => {
    if (video.availability && video.availability !== 'Available') {
      showNotification(`Cannot play ${video.availability.toLowerCase()} video`);
      return;
    }

    const startAt =
      typeof resumeSeconds === 'number'
        ? resumeSeconds
        : settings.resumePlayback && video.lastPositionSeconds
        ? video.lastPositionSeconds
        : 0;

    setSelectedVideo(video);
    setPlaybackQueue(queue || []);
    setIsPlaying(true);
    setCurrentTime(startAt);
    navigateTo('watch');

    // Record to persistent local history
    try {
      await fetch('/api/history', {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({
          ...video,
          lastPositionSeconds: startAt,
        }),
      });
      fetchHistory(historySearch);
    } catch {}

    // Load video stream formats & full description
    try {
      const res = await fetch(`/api/video/${encodeURIComponent(video.id)}`);
      if (res.ok) {
        const data = await res.json();
        setVideoDetail({
          description: data.description || video.description || '',
          availableQualities: data.availableQualities || ['1080p', '720p', '480p', '360p'],
        });
      }
    } catch {}
  };

  // Next video in sequential playlist queue
  const handlePlayNextInQueue = () => {
    if (!selectedVideo || playbackQueue.length === 0) return;
    const idx = playbackQueue.findIndex((v) => v.id === selectedVideo.id);
    if (idx !== -1 && idx + 1 < playbackQueue.length) {
      handleSelectVideo(playbackQueue[idx + 1], playbackQueue, 0);
    }
  };

  // Rate Video (Like / Unlike) — updates UI strictly after API confirmation
  const handleToggleLike = async (video: VideoItem) => {
    if (isMutating) return;
    setIsMutating(true);
    const currentlyLiked = likedVideos.some((v) => v.id === video.id);
    const targetRating = currentlyLiked ? 'none' : 'like';
    try {
      const res = await fetch(`/api/videos/${encodeURIComponent(video.id)}/rate`, {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({ rating: targetRating, video }),
      });
      if (!res.ok) throw new Error('Failed to update video rating');
      const data = await res.json();
      setLikedVideos(data.likedVideos || []);
      await fetchAuthAndSyncStatus();
      showNotification(data.liked ? 'Added to Liked Videos' : 'Removed from Liked Videos');
    } catch (err: any) {
      setErrorMessage(err.message);
    } finally {
      setIsMutating(false);
    }
  };

  // Toggle Local RustTube Watch Later
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
        setLocalWatchLater(data.localWatchLater || []);
        showNotification(
          data.saved
            ? 'Saved to RustTube Local Watch Later'
            : 'Removed from RustTube Local Watch Later'
        );
      }
    } catch {
      showNotification('Could not update Watch Later');
    }
  };

  // Experimental YouTube Cookie Watch Later Import
  const handleExperimentalWatchLaterImport = async () => {
    setIsMutating(true);
    setErrorMessage('');
    try {
      const res = await fetch('/api/watch_later/experimental_import', {
        method: 'POST',
      });
      const data = await res.json();
      if (!res.ok) throw new Error(data.error || 'Cookie Watch Later import failed');
      setExperimentalWatchLater(data.experimentalYouTubeWatchLater || []);
      showNotification('Imported YouTube Watch Later via browser cookies without overwriting local queue');
    } catch (err: any) {
      setErrorMessage(err.message);
    } finally {
      setIsMutating(false);
    }
  };

  // History Operations (Single Removal & Clear All)
  const handleRemoveHistoryEntry = async (videoId: string, e?: React.MouseEvent) => {
    e?.stopPropagation();
    try {
      const res = await fetch(`/api/history/${encodeURIComponent(videoId)}`, {
        method: 'DELETE',
      });
      if (res.ok) {
        const data = await res.json();
        setHistoryList(data.history || []);
        showNotification('Removed entry from local watch history');
      }
    } catch {}
  };

  const handleClearHistory = async () => {
    try {
      const res = await fetch('/api/history', { method: 'DELETE' });
      if (res.ok) {
        setHistoryList([]);
        showNotification('Cleared all local watch history');
      }
    } catch {}
  };

  // Account Login / Logout / Manual Sync
  const handleLogin = async () => {
    setIsMutating(true);
    setErrorMessage('');
    try {
      const res = await fetch('/api/auth/login', {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({
          accessToken: oauthAccessTokenInput.trim() || undefined,
        }),
      });
      const data = await res.json();
      if (!res.ok) {
        if (data.classified) setClassifiedError(data.classified);
        throw new Error(data.error || 'OAuth sign-in failed');
      }
      setOauthAccessTokenInput('');
      await fetchAuthAndSyncStatus();
      await fetchSubscriptions();
      await fetchPlaylists();
      await fetchLikedVideos();
      showNotification('Signed in and restored Google OAuth2 session');
    } catch (err: any) {
      setErrorMessage(err.message);
    } finally {
      setIsMutating(false);
    }
  };

  const handleLogout = async () => {
    try {
      const res = await fetch('/api/auth/logout', { method: 'POST' });
      if (res.ok) {
        await fetchAuthAndSyncStatus();
        showNotification('Signed out and cleared Credential Manager tokens');
      }
    } catch {}
  };

  const handleManualSync = async () => {
    if (isSyncing) return;
    setIsSyncing(true);
    setErrorMessage('');
    setClassifiedError(null);
    try {
      const res = await fetch('/api/sync/run', { method: 'POST' });
      const data = await res.json();
      if (!res.ok) {
        if (data.classified) setClassifiedError(data.classified);
        throw new Error(data.error || 'Account synchronization failed');
      }
      setSyncState(data.sync);
      await fetchSubscriptions(subscriptionsFilter);
      await fetchPlaylists();
      await fetchLikedVideos();
      await fetchHomeFeed(feedCategory, 1, true);
      showNotification('Synchronized Subscriptions, Playlists, and Liked Videos');
    } catch (err: any) {
      setErrorMessage(err.message);
    } finally {
      setIsSyncing(false);
    }
  };

  const handleSaveSettings = async (next: Partial<AppSettings>) => {
    const merged = { ...settings, ...next };
    setSettings(merged);
    try {
      await fetch('/api/settings', {
        method: 'PUT',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify(merged),
      });
      showNotification('Saved configuration to %APPDATA%\\RustTube\\config.toml');
    } catch {}
  };

  // Native Updater Actions
  const handleCheckUpdates = async () => {
    setUpdater((prev) => ({ ...prev, status: 'checking', errorMessage: null }));
    try {
      const res = await fetch('/api/updater/check', { method: 'POST' });
      const data = await res.json();
      if (!res.ok) throw new Error(data.error || 'Update check failed');
      setUpdater(data.updater);
      showNotification(
        data.updateAvailable
          ? `Update v${data.updater.latestRelease?.version} available!`
          : `RustTube v${data.updater.currentVersion} is up to date`
      );
    } catch (err: any) {
      setErrorMessage(err.message);
    }
  };

  const handleStageUpdate = async () => {
    setIsMutating(true);
    try {
      const res = await fetch('/api/updater/stage', {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({
          version: updater.latestRelease?.version || '0.2.1',
        }),
      });
      const data = await res.json();
      if (!res.ok) throw new Error(data.error || 'Staging failed');
      setUpdater(data.updater);
      showNotification(`Staged update and verified SHA-256 checksum (${data.sha256.slice(0, 12)}...)`);
    } catch (err: any) {
      setErrorMessage(err.message);
    } finally {
      setIsMutating(false);
    }
  };

  const handleApplyUpdate = async () => {
    setIsMutating(true);
    try {
      const res = await fetch('/api/updater/apply', { method: 'POST' });
      const data = await res.json();
      if (!res.ok) throw new Error(data.error || 'Apply update failed');
      setUpdater(data.updater);
      showNotification(
        `Applied update via rusttube-updater helper. All user data (${data.preservedDataSummary.watchLaterPreserved} Watch Later, ${data.preservedDataSummary.historyPreserved} History) preserved.`
      );
    } catch (err: any) {
      setErrorMessage(err.message);
    } finally {
      setIsMutating(false);
    }
  };

  // Periodically save playback progress to local history so Continue Playback works
  useEffect(() => {
    if (!isPlaying || !selectedVideo) return;
    const timer = setInterval(() => {
      setCurrentTime((prev) => {
        const next = prev + 1;
        if (next % 10 === 0) {
          fetch('/api/history', {
            method: 'POST',
            headers: { 'Content-Type': 'application/json' },
            body: JSON.stringify({
              ...selectedVideo,
              lastPositionSeconds: next,
            }),
          }).catch(() => {});
        }
        return next;
      });
    }, Math.max(250, Math.round(1000 / speed)));
    return () => clearInterval(timer);
  }, [isPlaying, selectedVideo, speed]);

  const formatSeconds = (sec: number) => {
    const m = Math.floor(sec / 60);
    const s = Math.floor(sec % 60);
    return `${m}:${s < 10 ? '0' : ''}${s}`;
  };

  const renderVideoCard = (video: VideoItem, idx: number, contextQueue?: VideoItem[]) => {
    const isSelected = selectedVideo?.id === video.id;
    const inWatchLater = localWatchLater.some((v) => v.id === video.id);
    const isLiked = likedVideos.some((v) => v.id === video.id);
    const showPlaylistMenu = playlistDropdownVideoId === video.id;

    return (
      <div
        key={`card-${video.id}-${idx}`}
        onClick={() => handleSelectVideo(video, contextQueue, video.lastPositionSeconds)}
        className={`group relative cursor-pointer rounded-xl bg-[#12131a] border transition-colors duration-150 flex flex-col ${
          isSelected
            ? 'border-[#ff0033]/70 bg-[#161822]'
            : 'border-[#1d1f2b] hover:border-[#2e3246] hover:bg-[#151722]'
        }`}
      >
        {/* Thumbnail Container */}
        <div className="relative aspect-video w-full bg-[#181a24] rounded-t-xl overflow-hidden">
          <img
            src={video.thumbnail || `https://i.ytimg.com/vi/${video.id}/hqdefault.jpg`}
            alt={video.title}
            referrerPolicy="no-referrer"
            onError={(e) => {
              (e.currentTarget as HTMLImageElement).src =
                '/assets/aistudio/image_9995d559.png';
            }}
            className="w-full h-full object-cover group-hover:scale-105 transition-transform duration-200"
            loading="lazy"
          />
          <div className="absolute inset-0 bg-gradient-to-t from-black/75 via-transparent to-transparent opacity-60" />

          {/* Duration (Tabular Numerals) */}
          {video.duration && (
            <span className="absolute bottom-2 right-2 bg-black/85 text-white text-[11px] font-mono tabular-nums px-1.5 py-0.5 rounded">
              {video.duration}
            </span>
          )}

          {/* Resume Progress Bar if in History */}
          {typeof video.lastPositionSeconds === 'number' &&
            video.lastPositionSeconds > 0 &&
            video.durationSeconds > 0 && (
              <div className="absolute bottom-0 left-0 right-0 h-1 bg-white/20">
                <div
                  className="h-full bg-[#ff0033]"
                  style={{
                    width: `${Math.min(
                      100,
                      Math.round((video.lastPositionSeconds / video.durationSeconds) * 100)
                    )}%`,
                  }}
                />
              </div>
            )}

          {/* Quick Hover Actions: Watch Later & Add to Playlist */}
          <div
            className="absolute top-2 right-2 flex items-center gap-1.5 opacity-0 group-hover:opacity-100 transition-opacity"
            onClick={(e) => e.stopPropagation()}
          >
            <button
              type="button"
              onClick={(e) => handleToggleWatchLater(video, e)}
              title={inWatchLater ? 'Remove from Local Watch Later' : 'Save to Local Watch Later'}
              className="p-1.5 rounded-md bg-black/80 hover:bg-black text-white border border-white/10"
            >
              {inWatchLater ? (
                <BookmarkCheck className="w-3.5 h-3.5 text-[#ff4d6d]" />
              ) : (
                <Bookmark className="w-3.5 h-3.5" />
              )}
            </button>
            <button
              type="button"
              onClick={() =>
                setPlaylistDropdownVideoId(showPlaylistMenu ? null : video.id)
              }
              title="Add to Playlist"
              className="p-1.5 rounded-md bg-black/80 hover:bg-black text-white border border-white/10"
            >
              <Plus className="w-3.5 h-3.5" />
            </button>
          </div>
        </div>

        {/* Add-to-Playlist Popover Menu */}
        {showPlaylistMenu && (
          <div
            onClick={(e) => e.stopPropagation()}
            className="absolute top-10 right-2 z-30 w-52 rounded-lg bg-[#181a26] border border-[#2c3044] shadow-2xl p-2 space-y-1"
          >
            <div className="flex items-center justify-between px-2 py-1 border-b border-[#252838]">
              <span className="text-[11px] font-semibold text-white">Add to Playlist</span>
              <button
                type="button"
                onClick={() => setPlaylistDropdownVideoId(null)}
                className="text-[#8a90a6] hover:text-white"
              >
                <X className="w-3 h-3" />
              </button>
            </div>
            {playlists.length === 0 ? (
              <p className="text-[11px] text-[#8a90a6] px-2 py-1.5">No playlists created yet.</p>
            ) : (
              playlists.map((pl) => (
                <button
                  key={pl.playlistId}
                  type="button"
                  onClick={() => handleAddVideoToPlaylist(pl.playlistId, video)}
                  className="w-full text-left px-2 py-1.5 rounded text-xs text-[#d0d4e6] hover:bg-[#232738] hover:text-white truncate flex items-center justify-between"
                >
                  <span className="truncate">{pl.title}</span>
                  <span className="text-[10px] font-mono tabular-nums text-[#7b8094]">
                    {pl.itemCount}
                  </span>
                </button>
              ))
            )}
          </div>
        )}

        {/* Card Body — Clean Unboxed Metadata with Middot Separators */}
        <div className="p-3.5 flex flex-col justify-between flex-1 gap-2">
          <h3 className="text-xs font-semibold text-[#eef0f8] line-clamp-2 leading-snug group-hover:text-[#ff4d6d] transition-colors">
            {video.title}
          </h3>

          <div className="space-y-1">
            <div className="flex items-center gap-1.5 text-[11px] text-[#9ea3b8]">
              <button
                type="button"
                onClick={(e) => {
                  e.stopPropagation();
                  handleOpenChannel(video.channelId || video.channel);
                }}
                className="hover:text-white hover:underline truncate max-w-[180px] text-left font-medium"
              >
                {video.channel}
              </button>
              {isLiked && (
                <>
                  <span aria-hidden="true">·</span>
                  <span className="text-[#ff4d6d]">Liked</span>
                </>
              )}
            </div>

            <div className="flex items-center gap-1.5 text-[11px] text-[#757a90] font-mono tabular-nums">
              <span>{video.views || 'Available'}</span>
              {video.published && (
                <>
                  <span aria-hidden="true">·</span>
                  <span>{video.published}</span>
                </>
              )}
              {video.sourceTag && (
                <>
                  <span aria-hidden="true">·</span>
                  <span className="font-sans text-[#8a90a6] truncate">{video.sourceTag}</span>
                </>
              )}
            </div>
          </div>
        </div>
      </div>
    );
  };

  return (
    <div className="flex h-screen w-screen overflow-hidden bg-[#0b0c10] text-[#e0e2ec] select-none">
      {/* Toast Notification */}
      {notification && (
        <div className="fixed top-4 right-6 z-50 flex items-center gap-2 bg-[#171924] border border-[#2b3044] text-[#e0e2ec] px-4 py-2.5 rounded-lg shadow-xl text-xs font-medium">
          <CheckCircle2 className="w-4 h-4 text-emerald-400 shrink-0" />
          <span>{notification}</span>
        </div>
      )}

      <div className="flex flex-col flex-1 h-full overflow-hidden">
        {/* Top Header Bar */}
        <header className="h-14 bg-[#101218] border-b border-[#1c1f2b] px-5 flex items-center justify-between gap-6 shrink-0">
          {/* Zone 1: Back Navigation & Brand Wordmark */}
          <div className="flex items-center gap-3 shrink-0">
            <button
              type="button"
              disabled={navStack.length === 0}
              onClick={navigateBack}
              title="Navigate Back"
              className="p-1.5 rounded-md bg-[#171923] border border-[#242838] text-[#9ea3b8] hover:text-white disabled:opacity-35 disabled:pointer-events-none transition-colors"
            >
              <ArrowLeft className="w-4 h-4" />
            </button>
            <button
              type="button"
              onClick={() => {
                navigateTo('home');
                fetchHomeFeed(feedCategory, 1, false);
              }}
              className="flex items-center gap-2.5 text-left whitespace-nowrap shrink-0"
            >
              <div className="flex items-center justify-center w-7 h-7 rounded-md bg-[#ff0033]">
                <Play className="w-3.5 h-3.5 text-white fill-white ml-0.5" />
              </div>
              <span className="font-bold text-white text-base tracking-tight">RustTube</span>
            </button>
          </div>

          {/* Zone 2: Search Bar with Direct URL Resolution */}
          <div className="flex-1 max-w-xl">
            <form
              onSubmit={(e) => {
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
                onChange={(e) => setSearchQuery(e.target.value)}
                placeholder="Search videos, channels, or paste YouTube URL / ID..."
                className="w-full bg-[#161822] border border-[#252838] rounded-lg pl-10 pr-9 py-2 text-xs text-white placeholder-[#666a7e] focus:outline-none focus:border-[#ff0033]/80 transition-colors"
              />
              {searchQuery && (
                <button
                  type="button"
                  onClick={() => {
                    setSearchQuery('');
                    if (currentPage === 'search') {
                      navigateTo('home');
                    }
                  }}
                  className="absolute right-3 top-1/2 -translate-y-1/2 text-[#82869a] hover:text-white"
                >
                  <X className="w-3.5 h-3.5" />
                </button>
              )}
            </form>
          </div>

          {/* Zone 3: Sync Action & Account Control */}
          <div className="flex items-center gap-3 shrink-0">
            <button
              type="button"
              disabled={isSyncing}
              onClick={handleManualSync}
              className="flex items-center gap-1.5 px-3 py-1.5 rounded-md bg-[#171924] hover:bg-[#1e2130] border border-[#272b3d] text-xs font-medium text-[#c8cde0] hover:text-white disabled:opacity-50 whitespace-nowrap shrink-0 transition-colors"
            >
              <RefreshCw className={`w-3.5 h-3.5 ${isSyncing ? 'animate-spin text-[#ff4d6d]' : ''}`} />
              <span>{isSyncing ? 'Syncing...' : 'Sync Account'}</span>
            </button>

            {auth.isLoggedIn ? (
              <button
                type="button"
                onClick={() => navigateTo('settings')}
                className="flex items-center gap-2 px-2.5 py-1 rounded-md hover:bg-[#171924] transition-colors whitespace-nowrap shrink-0"
              >
                <div className="w-7 h-7 rounded-full bg-[#202334] border border-[#30344c] overflow-hidden flex items-center justify-center">
                  <img
                    src={auth.avatarUrl || '/assets/aistudio/image_9995d559.png'}
                    alt={auth.userName}
                    referrerPolicy="no-referrer"
                    className="w-full h-full object-cover"
                  />
                </div>
                <span className="text-xs font-medium text-white max-w-[130px] truncate hidden sm:block">
                  {auth.userName}
                </span>
              </button>
            ) : (
              <button
                type="button"
                onClick={handleLogin}
                className="px-3.5 py-1.5 bg-[#ff0033] hover:bg-[#d9002c] text-white text-xs font-semibold rounded-md transition-colors whitespace-nowrap shrink-0"
              >
                Sign In
              </button>
            )}
          </div>
        </header>

        {/* Main Workspace Body */}
        <div className="flex flex-1 overflow-hidden">
          {/* Navigation Sidebar */}
          <aside className="w-56 bg-[#0e1016] border-r border-[#1a1c26] p-3 flex flex-col justify-between shrink-0">
            <div className="space-y-1">
              <button
                type="button"
                onClick={() => {
                  navigateTo('home');
                  fetchHomeFeed(feedCategory, 1, false);
                }}
                className={`w-full flex items-center justify-between px-3 py-2 rounded-lg text-xs font-medium transition-colors whitespace-nowrap ${
                  currentPage === 'home'
                    ? 'bg-[#ff0033]/15 text-[#ff4d6d] font-semibold border border-[#ff0033]/30'
                    : 'text-[#9fa3b8] hover:bg-[#151722] hover:text-white'
                }`}
              >
                <span className="flex items-center gap-3">
                  <Tv className="w-4 h-4" />
                  <span>Home</span>
                </span>
              </button>

              <button
                type="button"
                onClick={() => {
                  navigateTo('subscriptions');
                  fetchSubscriptions(subscriptionsFilter);
                }}
                className={`w-full flex items-center justify-between px-3 py-2 rounded-lg text-xs font-medium transition-colors whitespace-nowrap ${
                  currentPage === 'subscriptions' || currentPage === 'channel'
                    ? 'bg-[#ff0033]/15 text-[#ff4d6d] font-semibold border border-[#ff0033]/30'
                    : 'text-[#9fa3b8] hover:bg-[#151722] hover:text-white'
                }`}
              >
                <span className="flex items-center gap-3">
                  <ListVideo className="w-4 h-4" />
                  <span>Subscriptions</span>
                </span>
                <span className="text-[11px] font-mono tabular-nums text-[#757a90]">
                  {subscriptions.length}
                </span>
              </button>

              <button
                type="button"
                onClick={() => {
                  navigateTo('playlists');
                  fetchPlaylists();
                }}
                className={`w-full flex items-center justify-between px-3 py-2 rounded-lg text-xs font-medium transition-colors whitespace-nowrap ${
                  currentPage === 'playlists' || currentPage === 'playlist_detail'
                    ? 'bg-[#ff0033]/15 text-[#ff4d6d] font-semibold border border-[#ff0033]/30'
                    : 'text-[#9fa3b8] hover:bg-[#151722] hover:text-white'
                }`}
              >
                <span className="flex items-center gap-3">
                  <FolderTree className="w-4 h-4" />
                  <span>Playlists</span>
                </span>
                <span className="text-[11px] font-mono tabular-nums text-[#757a90]">
                  {playlists.length}
                </span>
              </button>

              <button
                type="button"
                onClick={() => {
                  navigateTo('liked');
                  fetchLikedVideos();
                }}
                className={`w-full flex items-center justify-between px-3 py-2 rounded-lg text-xs font-medium transition-colors whitespace-nowrap ${
                  currentPage === 'liked'
                    ? 'bg-[#ff0033]/15 text-[#ff4d6d] font-semibold border border-[#ff0033]/30'
                    : 'text-[#9fa3b8] hover:bg-[#151722] hover:text-white'
                }`}
              >
                <span className="flex items-center gap-3">
                  <ThumbsUp className="w-4 h-4" />
                  <span>Liked Videos</span>
                </span>
                <span className="text-[11px] font-mono tabular-nums text-[#757a90]">
                  {likedVideos.length}
                </span>
              </button>

              <button
                type="button"
                onClick={() => {
                  navigateTo('watch_later');
                  fetchWatchLater();
                }}
                className={`w-full flex items-center justify-between px-3 py-2 rounded-lg text-xs font-medium transition-colors whitespace-nowrap ${
                  currentPage === 'watch_later'
                    ? 'bg-[#ff0033]/15 text-[#ff4d6d] font-semibold border border-[#ff0033]/30'
                    : 'text-[#9fa3b8] hover:bg-[#151722] hover:text-white'
                }`}
              >
                <span className="flex items-center gap-3">
                  <Clock className="w-4 h-4" />
                  <span>Watch Later</span>
                </span>
                <span className="text-[11px] font-mono tabular-nums text-[#757a90]">
                  {localWatchLater.length}
                </span>
              </button>

              <button
                type="button"
                onClick={() => {
                  navigateTo('history');
                  fetchHistory(historySearch);
                }}
                className={`w-full flex items-center justify-between px-3 py-2 rounded-lg text-xs font-medium transition-colors whitespace-nowrap ${
                  currentPage === 'history'
                    ? 'bg-[#ff0033]/15 text-[#ff4d6d] font-semibold border border-[#ff0033]/30'
                    : 'text-[#9fa3b8] hover:bg-[#151722] hover:text-white'
                }`}
              >
                <span className="flex items-center gap-3">
                  <History className="w-4 h-4" />
                  <span>History</span>
                </span>
                <span className="text-[11px] font-mono tabular-nums text-[#757a90]">
                  {historyList.length}
                </span>
              </button>
            </div>

            {/* Bottom Settings & Updater Navigation */}
            <div className="pt-3 border-t border-[#1a1c26] space-y-2">
              <div className="px-3 py-1 text-[11px] text-[#6d7288] font-mono tabular-nums">
                <div>v{updater.currentVersion} · Windows x64</div>
                {syncState.lastSuccessfulSync && (
                  <div className="truncate">
                    Synced {new Date(syncState.lastSuccessfulSync).toLocaleTimeString([], { hour: '2-digit', minute: '2-digit' })}
                  </div>
                )}
              </div>
              <button
                type="button"
                onClick={() => navigateTo('settings')}
                className={`w-full flex items-center justify-between px-3 py-2 rounded-lg text-xs font-medium transition-colors whitespace-nowrap ${
                  currentPage === 'settings'
                    ? 'bg-[#ff0033]/15 text-[#ff4d6d] font-semibold border border-[#ff0033]/30'
                    : 'text-[#9fa3b8] hover:bg-[#151722] hover:text-white'
                }`}
              >
                <span className="flex items-center gap-3">
                  <Settings className="w-4 h-4" />
                  <span>Settings & Updater</span>
                </span>
              </button>
            </div>
          </aside>

          {/* Main Content Viewport */}
          <main className="flex-1 flex flex-col overflow-hidden bg-[#0b0c10]">
            <div className="flex-1 overflow-y-auto p-6">
              {/* Classified Error Banner */}
              {(errorMessage || classifiedError) && (
                <div className="mb-5 p-3.5 bg-red-950/35 border border-red-800/50 rounded-lg flex items-center justify-between gap-3 text-xs text-red-200">
                  <div className="flex items-center gap-2.5">
                    <AlertCircle className="w-4 h-4 shrink-0 text-red-400" />
                    <div>
                      {classifiedError && (
                        <span className="font-mono font-semibold text-red-300 mr-2">
                          [{classifiedError.kind}]
                        </span>
                      )}
                      <span>{errorMessage || classifiedError?.message}</span>
                    </div>
                  </div>
                  <button
                    type="button"
                    onClick={() => {
                      setErrorMessage('');
                      setClassifiedError(null);
                    }}
                    className="text-red-300 hover:text-white"
                  >
                    <X className="w-3.5 h-3.5" />
                  </button>
                </div>
              )}

              {/* VIEW 1: HOME (Personalized Feed & Category Filter Bar) */}
              {currentPage === 'home' && (
                <div className="space-y-5">
                  <div className="flex flex-wrap items-center justify-between gap-4">
                    <div>
                      <h1 className="text-lg font-bold text-white">
                        {feedCategory === 'personalized' && 'Personalized Account Feed'}
                        {feedCategory === 'subscriptions' && 'Latest from Subscribed Channels'}
                        {feedCategory === 'liked_related' && 'Relevant to Your Liked Videos'}
                        {feedCategory === 'continue_watching' && 'Continue Watching (Local History)'}
                        {feedCategory === 'trending' && 'Popular & Trending'}
                        {feedCategory === 'experimental_cookie' &&
                          'Experimental Cookie Homepage Provider'}
                      </h1>
                      <p className="text-xs text-[#7b8094]">
                        {feedCategory === 'personalized'
                          ? 'Built from your subscribed channel uploads (playlistItems.list), liked video topics, and local watch history.'
                          : feedCategory === 'experimental_cookie'
                          ? 'Opt-in yt-dlp browser cookie extraction with automatic local fallback.'
                          : 'Sorted by publication time with duplicate removal and local caching.'}
                      </p>
                    </div>

                    <button
                      type="button"
                      disabled={isLoading}
                      onClick={() => fetchHomeFeed(feedCategory, 1, true)}
                      className="flex items-center gap-1.5 px-3 py-1.5 rounded-md bg-[#151722] hover:bg-[#1c1f2e] border border-[#252838] text-xs font-medium text-[#d0d4e6] hover:text-white whitespace-nowrap shrink-0"
                    >
                      <RefreshCw className={`w-3.5 h-3.5 ${isLoading ? 'animate-spin' : ''}`} />
                      <span>Refresh Feed</span>
                    </button>
                  </div>

                  {/* Interactive Category Filter Buttons */}
                  <div className="flex flex-wrap items-center gap-1.5 p-1 bg-[#12141d] border border-[#1d202d] rounded-lg w-fit">
                    {(
                      [
                        ['personalized', 'Personalized Feed'],
                        ['subscriptions', 'Subscribed Uploads'],
                        ['liked_related', 'Based on Liked'],
                        ['continue_watching', 'Continue Watching'],
                        ['trending', 'Popular / Trending'],
                        ['experimental_cookie', 'Cookie Home (Experimental)'],
                      ] as [FeedCategory, string][]
                    ).map(([cat, label]) => (
                      <button
                        key={cat}
                        type="button"
                        onClick={() => {
                          setFeedCategory(cat);
                          fetchHomeFeed(cat, 1, false);
                        }}
                        className={`px-3 py-1.5 rounded-md text-xs font-medium transition-colors whitespace-nowrap shrink-0 ${
                          feedCategory === cat
                            ? 'bg-[#ff0033] text-white font-semibold'
                            : 'text-[#9ea3b8] hover:text-white hover:bg-[#191c28]'
                        }`}
                      >
                        {label}
                      </button>
                    ))}
                  </div>

                  {isLoading && homeVideos.length === 0 ? (
                    <div className="py-24 flex flex-col items-center justify-center gap-3">
                      <Loader2 className="w-7 h-7 text-[#ff0033] animate-spin" />
                      <p className="text-xs text-[#8c91a6]">
                        Resolving channel uploads and building personalized feed...
                      </p>
                    </div>
                  ) : homeVideos.length === 0 ? (
                    <div className="py-20 text-center space-y-3 bg-[#11131a] border border-[#1c1f2b] rounded-xl p-6">
                      <AlertCircle className="w-7 h-7 text-[#666a7e] mx-auto" />
                      <p className="text-sm text-[#b0b5c9]">
                        {feedCategory === 'continue_watching'
                          ? 'No previously watched videos in your local history yet.'
                          : feedCategory === 'experimental_cookie'
                          ? 'Experimental cookie homepage requires enabling the opt-in toggle and selecting a browser in Settings.'
                          : 'No videos available in this feed category.'}
                      </p>
                      {feedCategory === 'experimental_cookie' && (
                        <button
                          type="button"
                          onClick={() => navigateTo('settings')}
                          className="px-3.5 py-1.5 bg-[#ff0033] text-white text-xs font-semibold rounded-md"
                        >
                          Configure Cookie Provider in Settings
                        </button>
                      )}
                    </div>
                  ) : (
                    <>
                      <div className="grid grid-cols-1 md:grid-cols-2 lg:grid-cols-3 xl:grid-cols-4 gap-4">
                        {homeVideos.map((v, i) => renderVideoCard(v, i, homeVideos))}
                      </div>
                      {feedHasMore && (
                        <div className="pt-4 flex justify-center">
                          <button
                            type="button"
                            disabled={isLoading}
                            onClick={() => fetchHomeFeed(feedCategory, feedPage + 1, false)}
                            className="px-4 py-2 rounded-lg bg-[#161822] hover:bg-[#1e2130] border border-[#272b3d] text-xs font-semibold text-white"
                          >
                            {isLoading ? 'Loading More...' : 'Load More Videos'}
                          </button>
                        </div>
                      )}
                    </>
                  )}
                </div>
              )}

              {/* VIEW 2: SUBSCRIPTIONS (Interactive Channel Cards, Search, Refresh, Unsubscribe) */}
              {currentPage === 'subscriptions' && (
                <div className="space-y-5">
                  <div className="flex flex-wrap items-center justify-between gap-4">
                    <div>
                      <h1 className="text-lg font-bold text-white">
                        Subscribed Channels ({subscriptions.length})
                      </h1>
                      <p className="text-xs text-[#7b8094]">
                        Click any channel card to open its dedicated Channel Page and browse its uploads playlist.
                      </p>
                    </div>

                    <div className="flex items-center gap-2.5">
                      <div className="relative">
                        <Search className="w-3.5 h-3.5 text-[#666a7e] absolute left-3 top-1/2 -translate-y-1/2" />
                        <input
                          type="text"
                          value={subscriptionsFilter}
                          onChange={(e) => {
                            setSubscriptionsFilter(e.target.value);
                            fetchSubscriptions(e.target.value);
                          }}
                          placeholder="Filter subscribed channels..."
                          className="bg-[#151722] border border-[#252838] rounded-lg pl-8 pr-3 py-1.5 text-xs text-white placeholder-[#666a7e] focus:outline-none focus:border-[#ff0033]"
                        />
                      </div>
                      <button
                        type="button"
                        onClick={() => fetchSubscriptions(subscriptionsFilter)}
                        className="flex items-center gap-1.5 px-3 py-1.5 rounded-lg bg-[#151722] hover:bg-[#1c1f2e] border border-[#252838] text-xs font-medium text-white whitespace-nowrap shrink-0"
                      >
                        <RefreshCw className="w-3.5 h-3.5" />
                        <span>Refresh</span>
                      </button>
                    </div>
                  </div>

                  {!auth.isLoggedIn ? (
                    <div className="py-20 text-center space-y-3 bg-[#11131a] border border-[#1c1f2b] rounded-xl">
                      <ListVideo className="w-8 h-8 text-[#54586d] mx-auto" />
                      <p className="text-sm text-[#8c91a6]">
                        Sign in with Google to view and manage your subscribed channels.
                      </p>
                      <button
                        type="button"
                        onClick={handleLogin}
                        className="px-4 py-2 bg-[#ff0033] hover:bg-[#d9002c] text-white text-xs font-semibold rounded-md"
                      >
                        Sign In with Google
                      </button>
                    </div>
                  ) : subscriptions.length === 0 ? (
                    <div className="py-16 text-center space-y-2 bg-[#11131a] border border-[#1c1f2b] rounded-xl">
                      <p className="text-sm text-[#8c91a6]">
                        No subscribed channels match your filter.
                      </p>
                    </div>
                  ) : (
                    <div className="grid grid-cols-1 md:grid-cols-2 gap-4">
                      {subscriptions.map((ch) => (
                        <div
                          key={ch.channelId}
                          onClick={() => handleOpenChannel(ch.channelId)}
                          className="group cursor-pointer p-4 rounded-xl bg-[#12141c] border border-[#1e2130] hover:border-[#32374f] hover:bg-[#161924] transition-colors flex items-start justify-between gap-4"
                        >
                          <div className="flex items-start gap-3.5 min-w-0">
                            <img
                              src={ch.avatarUrl}
                              alt={ch.title}
                              referrerPolicy="no-referrer"
                              onError={(e) => {
                                (e.currentTarget as HTMLImageElement).src =
                                  '/assets/aistudio/image_9995d559.png';
                              }}
                              className="w-12 h-12 rounded-full object-cover border border-[#2a2e42] shrink-0"
                            />
                            <div className="min-w-0 space-y-1">
                              <div className="flex items-center gap-2">
                                <h3 className="text-sm font-bold text-white group-hover:text-[#ff4d6d] truncate">
                                  {ch.title}
                                </h3>
                                <ChevronRight className="w-3.5 h-3.5 text-[#666a7e] group-hover:text-white shrink-0" />
                              </div>
                              <div className="flex items-center gap-1.5 text-[11px] text-[#7b8094] font-mono tabular-nums">
                                <span>{ch.handle}</span>
                                <span aria-hidden="true">·</span>
                                <span>{ch.subscriberCount} subscribers</span>
                                <span aria-hidden="true">·</span>
                                <span>Uploads: {ch.uploadsPlaylistId}</span>
                              </div>
                              <p className="text-xs text-[#9ea3b8] line-clamp-2">
                                {ch.description}
                              </p>
                            </div>
                          </div>

                          <div
                            className="shrink-0 flex items-center gap-2"
                            onClick={(e) => e.stopPropagation()}
                          >
                            {confirmUnsubscribeId === ch.channelId ? (
                              <div className="flex items-center gap-1.5">
                                <button
                                  type="button"
                                  disabled={isMutating}
                                  onClick={() => handleUnsubscribeChannel(ch.channelId)}
                                  className="px-2.5 py-1 rounded bg-red-600 hover:bg-red-500 text-white text-[11px] font-semibold whitespace-nowrap"
                                >
                                  Confirm
                                </button>
                                <button
                                  type="button"
                                  onClick={() => setConfirmUnsubscribeId(null)}
                                  className="px-2 py-1 rounded bg-[#1d202d] text-[#9ea3b8] hover:text-white text-[11px]"
                                >
                                  Cancel
                                </button>
                              </div>
                            ) : (
                              <button
                                type="button"
                                onClick={() => setConfirmUnsubscribeId(ch.channelId)}
                                className="px-3 py-1.5 rounded-md bg-[#191c28] hover:bg-red-950/50 border border-[#282c3e] hover:border-red-800/50 text-xs text-[#a0a5ba] hover:text-red-300 transition-colors whitespace-nowrap"
                              >
                                Unsubscribe
                              </button>
                            )}
                          </div>
                        </div>
                      ))}
                    </div>
                  )}
                </div>
              )}

              {/* VIEW 3: DEDICATED CHANNEL PAGE */}
              {currentPage === 'channel' && (
                <div className="space-y-6">
                  {activeChannel ? (
                    <>
                      <div className="p-5 rounded-xl bg-[#12141c] border border-[#1e2130] flex flex-wrap items-start justify-between gap-6">
                        <div className="flex items-start gap-4 max-w-3xl">
                          <img
                            src={activeChannel.avatarUrl}
                            alt={activeChannel.title}
                            referrerPolicy="no-referrer"
                            onError={(e) => {
                              (e.currentTarget as HTMLImageElement).src =
                                '/assets/aistudio/image_9995d559.png';
                            }}
                            className="w-16 h-16 rounded-full object-cover border border-[#2e3246] shrink-0"
                          />
                          <div className="space-y-1.5">
                            <h1 className="text-xl font-bold text-white">
                              {activeChannel.title}
                            </h1>
                            <div className="flex flex-wrap items-center gap-2 text-xs text-[#8a90a6] font-mono tabular-nums">
                              <span>{activeChannel.handle}</span>
                              <span aria-hidden="true">·</span>
                              <span>{activeChannel.subscriberCount} subscribers</span>
                              <span aria-hidden="true">·</span>
                              <span>Uploads Playlist: {activeChannel.uploadsPlaylistId}</span>
                            </div>
                            <p className="text-xs text-[#b0b5c9] leading-relaxed">
                              {activeChannel.description}
                            </p>
                            <div className="pt-1 flex items-center gap-3 text-xs">
                              <a
                                href={activeChannel.channelUrl}
                                target="_blank"
                                rel="noreferrer"
                                className="inline-flex items-center gap-1 text-[#ff4d6d] hover:underline font-mono"
                              >
                                <span>{activeChannel.channelUrl}</span>
                                <ExternalLink className="w-3 h-3" />
                              </a>
                            </div>
                          </div>
                        </div>

                        <div className="flex items-center gap-2.5 shrink-0">
                          {activeChannelSubscribed ? (
                            <button
                              type="button"
                              disabled={isMutating}
                              onClick={() => handleUnsubscribeChannel(activeChannel.channelId)}
                              className="px-3.5 py-2 rounded-lg bg-[#191c28] hover:bg-red-950/50 border border-[#2b3044] text-xs font-semibold text-[#d0d4e6] hover:text-red-300 transition-colors whitespace-nowrap"
                            >
                              Subscribed · Unsubscribe
                            </button>
                          ) : (
                            <button
                              type="button"
                              disabled={isMutating}
                              onClick={() => handleSubscribeChannel(activeChannel)}
                              className="px-4 py-2 rounded-lg bg-[#ff0033] hover:bg-[#d9002c] text-xs font-semibold text-white transition-colors whitespace-nowrap"
                            >
                              Subscribe
                            </button>
                          )}
                          <button
                            type="button"
                            onClick={() => handleOpenChannel(activeChannel.channelId, 1)}
                            className="p-2 rounded-lg bg-[#171924] hover:bg-[#1f2232] border border-[#272b3d] text-[#9ea3b8] hover:text-white"
                            title="Refresh Channel Uploads"
                          >
                            <RefreshCw className="w-4 h-4" />
                          </button>
                        </div>
                      </div>

                      <div className="space-y-4">
                        <div className="flex items-center justify-between">
                          <h2 className="text-base font-bold text-white">
                            Latest Uploaded Videos ({channelVideos.length})
                          </h2>
                          {channelVideos.length > 0 && (
                            <button
                              type="button"
                              onClick={() =>
                                handleSelectVideo(channelVideos[0], channelVideos, 0)
                              }
                              className="flex items-center gap-1.5 px-3 py-1.5 rounded-lg bg-[#ff0033] hover:bg-[#d9002c] text-xs font-semibold text-white"
                            >
                              <Play className="w-3.5 h-3.5 fill-white" />
                              <span>Play All Uploads</span>
                            </button>
                          )}
                        </div>

                        <div className="grid grid-cols-1 md:grid-cols-2 lg:grid-cols-3 xl:grid-cols-4 gap-4">
                          {channelVideos.map((v, i) =>
                            renderVideoCard(v, i, channelVideos)
                          )}
                        </div>

                        {channelHasMore && (
                          <div className="pt-4 flex justify-center">
                            <button
                              type="button"
                              onClick={() =>
                                handleOpenChannel(activeChannel.channelId, channelPage + 1)
                              }
                              className="px-4 py-2 rounded-lg bg-[#161822] hover:bg-[#1e2130] border border-[#272b3d] text-xs font-semibold text-white"
                            >
                              Load More Channel Videos
                            </button>
                          </div>
                        )}
                      </div>
                    </>
                  ) : (
                    <div className="py-20 text-center text-xs text-[#8a90a6]">
                      Loading channel profile and uploads playlist...
                    </div>
                  )}
                </div>
              )}

              {/* VIEW 4: PLAYLISTS OVERVIEW (Create, Delete, Open Playlist) */}
              {currentPage === 'playlists' && (
                <div className="space-y-6">
                  <div className="flex items-center justify-between">
                    <div>
                      <h1 className="text-lg font-bold text-white">
                        Your Playlists ({playlists.length})
                      </h1>
                      <p className="text-xs text-[#7b8094]">
                        Create playlists, configure privacy settings, and click any playlist to manage or play its videos.
                      </p>
                    </div>
                    <button
                      type="button"
                      onClick={fetchPlaylists}
                      className="flex items-center gap-1.5 px-3 py-1.5 rounded-lg bg-[#151722] hover:bg-[#1c1f2e] border border-[#252838] text-xs font-medium text-white"
                    >
                      <RefreshCw className="w-3.5 h-3.5" />
                      <span>Refresh</span>
                    </button>
                  </div>

                  {/* Create Playlist Form */}
                  <form
                    onSubmit={handleCreatePlaylist}
                    className="p-4 rounded-xl bg-[#12141c] border border-[#1e2130] space-y-3"
                  >
                    <h2 className="text-xs font-semibold text-white">Create New Playlist</h2>
                    <div className="grid grid-cols-1 md:grid-cols-4 gap-3">
                      <input
                        type="text"
                        value={newPlaylistTitle}
                        onChange={(e) => setNewPlaylistTitle(e.target.value)}
                        placeholder="Playlist title (required)"
                        className="bg-[#171924] border border-[#262a3b] rounded-lg px-3 py-2 text-xs text-white placeholder-[#666a7e] focus:outline-none focus:border-[#ff0033]"
                      />
                      <input
                        type="text"
                        value={newPlaylistDesc}
                        onChange={(e) => setNewPlaylistDesc(e.target.value)}
                        placeholder="Description (optional)"
                        className="md:col-span-2 bg-[#171924] border border-[#262a3b] rounded-lg px-3 py-2 text-xs text-white placeholder-[#666a7e] focus:outline-none focus:border-[#ff0033]"
                      />
                      <div className="flex items-center gap-2">
                        <select
                          value={newPlaylistPrivacy}
                          onChange={(e) =>
                            setNewPlaylistPrivacy(
                              e.target.value as 'private' | 'unlisted' | 'public'
                            )
                          }
                          className="flex-1 bg-[#171924] border border-[#262a3b] rounded-lg px-2.5 py-2 text-xs text-white focus:outline-none"
                        >
                          <option value="private">Private</option>
                          <option value="unlisted">Unlisted</option>
                          <option value="public">Public</option>
                        </select>
                        <button
                          type="submit"
                          disabled={!newPlaylistTitle.trim() || isMutating}
                          className="px-4 py-2 rounded-lg bg-[#ff0033] hover:bg-[#d9002c] disabled:opacity-40 text-xs font-semibold text-white whitespace-nowrap shrink-0"
                        >
                          Create
                        </button>
                      </div>
                    </div>
                  </form>

                  {/* Playlist Cards Grid */}
                  <div className="grid grid-cols-1 md:grid-cols-2 lg:grid-cols-3 gap-4">
                    {playlists.map((pl) => (
                      <div
                        key={pl.playlistId}
                        onClick={() => handleOpenPlaylist(pl.playlistId)}
                        className="group cursor-pointer rounded-xl bg-[#12141c] border border-[#1e2130] hover:border-[#32374f] overflow-hidden flex flex-col justify-between transition-colors"
                      >
                        <div>
                          <div className="relative aspect-video bg-[#181a24] overflow-hidden">
                            <img
                              src={pl.thumbnailUrl}
                              alt={pl.title}
                              referrerPolicy="no-referrer"
                              onError={(e) => {
                                (e.currentTarget as HTMLImageElement).src =
                                  '/assets/aistudio/image_9995d559.png';
                              }}
                              className="w-full h-full object-cover group-hover:scale-105 transition-transform duration-200"
                            />
                            <div className="absolute inset-0 bg-gradient-to-t from-black/80 via-black/20 to-transparent" />
                            <div className="absolute bottom-2.5 left-3 right-3 flex items-center justify-between text-xs text-white font-mono tabular-nums">
                              <span>{pl.itemCount} videos</span>
                              <span className="flex items-center gap-1 capitalize">
                                {pl.privacyStatus === 'private' && <Lock className="w-3 h-3" />}
                                {pl.privacyStatus === 'unlisted' && <EyeOff className="w-3 h-3" />}
                                {pl.privacyStatus === 'public' && <Globe className="w-3 h-3" />}
                                {pl.privacyStatus}
                              </span>
                            </div>
                          </div>

                          <div className="p-4 space-y-1.5">
                            <h3 className="text-sm font-bold text-white group-hover:text-[#ff4d6d] transition-colors">
                              {pl.title}
                            </h3>
                            <p className="text-xs text-[#8a90a6] line-clamp-2">
                              {pl.description || 'No description'}
                            </p>
                          </div>
                        </div>

                        <div
                          className="px-4 pb-3 pt-2 border-t border-[#1b1e2b] flex items-center justify-between"
                          onClick={(e) => e.stopPropagation()}
                        >
                          <button
                            type="button"
                            onClick={() => handleOpenPlaylist(pl.playlistId)}
                            className="text-xs font-semibold text-[#ff4d6d] hover:underline"
                          >
                            Open Playlist →
                          </button>

                          {confirmDeletePlaylistId === pl.playlistId ? (
                            <div className="flex items-center gap-1.5">
                              <button
                                type="button"
                                onClick={() => handleDeletePlaylist(pl.playlistId)}
                                className="px-2 py-1 rounded bg-red-600 text-white text-[11px] font-semibold"
                              >
                                Confirm Delete
                              </button>
                              <button
                                type="button"
                                onClick={() => setConfirmDeletePlaylistId(null)}
                                className="px-2 py-1 rounded bg-[#1d202d] text-[#9ea3b8] text-[11px]"
                              >
                                Cancel
                              </button>
                            </div>
                          ) : (
                            <button
                              type="button"
                              onClick={() => setConfirmDeletePlaylistId(pl.playlistId)}
                              className="p-1.5 text-[#757a90] hover:text-red-400 rounded"
                              title="Delete Playlist"
                            >
                              <Trash2 className="w-3.5 h-3.5" />
                            </button>
                          )}
                        </div>
                      </div>
                    ))}
                  </div>
                </div>
              )}

              {/* VIEW 5: PLAYLIST DETAILS (Sequential Playback, Edit Metadata/Privacy, Reorder & Remove Videos) */}
              {currentPage === 'playlist_detail' && activePlaylist && (
                <div className="space-y-6">
                  <div className="p-5 rounded-xl bg-[#12141c] border border-[#1e2130] space-y-4">
                    <div className="flex flex-wrap items-start justify-between gap-4">
                      <div className="space-y-1">
                        <div className="flex items-center gap-2 text-xs text-[#8a90a6] font-mono tabular-nums">
                          <button
                            type="button"
                            onClick={() => navigateTo('playlists')}
                            className="hover:text-white hover:underline"
                          >
                            Playlists
                          </button>
                          <span>/</span>
                          <span className="capitalize">{activePlaylist.privacyStatus}</span>
                          <span>·</span>
                          <span>{playlistItems.length} videos</span>
                        </div>
                        <h1 className="text-xl font-bold text-white">{activePlaylist.title}</h1>
                        <p className="text-xs text-[#9ea3b8]">
                          {activePlaylist.description || 'No description provided.'}
                        </p>
                      </div>

                      <div className="flex items-center gap-2.5">
                        {playlistItems.some((i) => i.availability === 'Available') && (
                          <button
                            type="button"
                            onClick={() => {
                              const playableQueue = playlistItems
                                .filter((i) => i.availability === 'Available')
                                .map((i) => i.video);
                              if (playableQueue.length > 0) {
                                handleSelectVideo(playableQueue[0], playableQueue, 0);
                              }
                            }}
                            className="flex items-center gap-1.5 px-4 py-2 rounded-lg bg-[#ff0033] hover:bg-[#d9002c] text-xs font-semibold text-white whitespace-nowrap"
                          >
                            <Play className="w-3.5 h-3.5 fill-white" />
                            <span>Play All Sequentially</span>
                          </button>
                        )}
                        <button
                          type="button"
                          onClick={() => setIsEditingPlaylist(!isEditingPlaylist)}
                          className="flex items-center gap-1.5 px-3 py-2 rounded-lg bg-[#171924] hover:bg-[#1f2232] border border-[#272b3d] text-xs font-medium text-white whitespace-nowrap"
                        >
                          <Edit3 className="w-3.5 h-3.5" />
                          <span>{isEditingPlaylist ? 'Close Editor' : 'Edit Playlist'}</span>
                        </button>
                      </div>
                    </div>

                    {isEditingPlaylist && (
                      <form
                        onSubmit={handleUpdatePlaylist}
                        className="pt-3 border-t border-[#1e2130] grid grid-cols-1 md:grid-cols-4 gap-3"
                      >
                        <input
                          type="text"
                          value={editPlaylistTitle}
                          onChange={(e) => setEditPlaylistTitle(e.target.value)}
                          placeholder="Playlist title"
                          className="bg-[#171924] border border-[#272b3d] rounded-lg px-3 py-2 text-xs text-white"
                        />
                        <input
                          type="text"
                          value={editPlaylistDesc}
                          onChange={(e) => setEditPlaylistDesc(e.target.value)}
                          placeholder="Description"
                          className="md:col-span-2 bg-[#171924] border border-[#272b3d] rounded-lg px-3 py-2 text-xs text-white"
                        />
                        <div className="flex items-center gap-2">
                          <select
                            value={editPlaylistPrivacy}
                            onChange={(e) =>
                              setEditPlaylistPrivacy(
                                e.target.value as 'private' | 'unlisted' | 'public'
                              )
                            }
                            className="flex-1 bg-[#171924] border border-[#272b3d] rounded-lg px-2.5 py-2 text-xs text-white"
                          >
                            <option value="private">Private</option>
                            <option value="unlisted">Unlisted</option>
                            <option value="public">Public</option>
                          </select>
                          <button
                            type="submit"
                            className="px-3.5 py-2 rounded-lg bg-[#ff0033] text-white text-xs font-semibold whitespace-nowrap"
                          >
                            Save
                          </button>
                        </div>
                      </form>
                    )}
                  </div>

                  {/* Playlist Items List */}
                  <div className="space-y-2.5">
                    {playlistItems.length === 0 ? (
                      <div className="py-16 text-center bg-[#12141c] border border-[#1e2130] rounded-xl text-xs text-[#8a90a6]">
                        This playlist has no videos yet. Use the "+" button on any video card to add videos.
                      </div>
                    ) : (
                      playlistItems.map((entry, idx) => {
                        const isPlayable = entry.availability === 'Available';
                        const playableQueue = playlistItems
                          .filter((i) => i.availability === 'Available')
                          .map((i) => i.video);

                        return (
                          <div
                            key={entry.playlistItemId}
                            onClick={() => {
                              if (isPlayable) {
                                handleSelectVideo(entry.video, playableQueue, 0);
                              }
                            }}
                            className={`p-3 rounded-xl bg-[#12141c] border border-[#1e2130] flex items-center justify-between gap-4 ${
                              isPlayable
                                ? 'cursor-pointer hover:border-[#32374f] hover:bg-[#161924]'
                                : 'opacity-65'
                            }`}
                          >
                            <div className="flex items-center gap-3.5 min-w-0">
                              <span className="w-6 text-center text-xs font-mono tabular-nums text-[#6d7288]">
                                {idx + 1}
                              </span>
                              <div className="w-28 aspect-video rounded-lg bg-[#181a24] overflow-hidden shrink-0">
                                <img
                                  src={entry.video.thumbnail}
                                  alt={entry.video.title}
                                  referrerPolicy="no-referrer"
                                  onError={(e) => {
                                    (e.currentTarget as HTMLImageElement).src =
                                      '/assets/aistudio/image_9995d559.png';
                                  }}
                                  className="w-full h-full object-cover"
                                />
                              </div>
                              <div className="min-w-0 space-y-1">
                                <h4 className="text-xs font-semibold text-white truncate">
                                  {isPlayable
                                    ? entry.video.title
                                    : `[${entry.availability} Video] ${entry.video.title}`}
                                </h4>
                                <div className="flex items-center gap-2 text-[11px] text-[#7b8094] font-mono tabular-nums">
                                  <span className="font-sans">{entry.video.channel}</span>
                                  <span aria-hidden="true">·</span>
                                  <span>{entry.video.duration}</span>
                                </div>
                              </div>
                            </div>

                            <div
                              className="flex items-center gap-1.5 shrink-0"
                              onClick={(e) => e.stopPropagation()}
                            >
                              <button
                                type="button"
                                disabled={idx === 0}
                                onClick={() =>
                                  handleReorderPlaylistItem(
                                    activePlaylist.playlistId,
                                    entry.playlistItemId,
                                    'up'
                                  )
                                }
                                className="p-1.5 rounded bg-[#181b26] text-[#8a90a6] hover:text-white disabled:opacity-30"
                                title="Move Up"
                              >
                                <ArrowUp className="w-3.5 h-3.5" />
                              </button>
                              <button
                                type="button"
                                disabled={idx === playlistItems.length - 1}
                                onClick={() =>
                                  handleReorderPlaylistItem(
                                    activePlaylist.playlistId,
                                    entry.playlistItemId,
                                    'down'
                                  )
                                }
                                className="p-1.5 rounded bg-[#181b26] text-[#8a90a6] hover:text-white disabled:opacity-30"
                                title="Move Down"
                              >
                                <ArrowDown className="w-3.5 h-3.5" />
                              </button>
                              <button
                                type="button"
                                onClick={() =>
                                  handleRemovePlaylistItem(
                                    activePlaylist.playlistId,
                                    entry.playlistItemId
                                  )
                                }
                                className="p-1.5 rounded bg-[#181b26] text-[#8a90a6] hover:text-red-400"
                                title="Remove from Playlist"
                              >
                                <Trash2 className="w-3.5 h-3.5" />
                              </button>
                            </div>
                          </div>
                        );
                      })
                    )}
                  </div>
                </div>
              )}

              {/* VIEW 6: LIKED VIDEOS */}
              {currentPage === 'liked' && (
                <div className="space-y-5">
                  <div className="flex items-center justify-between">
                    <div>
                      <h1 className="text-lg font-bold text-white">
                        Liked Videos ({likedVideos.length})
                      </h1>
                      <p className="text-xs text-[#7b8094]">
                        Synchronized with your Google account and cached in local storage.
                      </p>
                    </div>
                    <button
                      type="button"
                      onClick={fetchLikedVideos}
                      className="flex items-center gap-1.5 px-3 py-1.5 rounded-lg bg-[#151722] hover:bg-[#1c1f2e] border border-[#252838] text-xs font-medium text-white"
                    >
                      <RefreshCw className="w-3.5 h-3.5" />
                      <span>Refresh</span>
                    </button>
                  </div>

                  {likedVideos.length === 0 ? (
                    <div className="py-20 text-center bg-[#12141c] border border-[#1e2130] rounded-xl text-xs text-[#8a90a6]">
                      No liked videos yet. Click the Like button on any video watch page to rate it.
                    </div>
                  ) : (
                    <div className="grid grid-cols-1 md:grid-cols-2 lg:grid-cols-3 xl:grid-cols-4 gap-4">
                      {likedVideos.map((v, i) => renderVideoCard(v, i, likedVideos))}
                    </div>
                  )}
                </div>
              )}

              {/* VIEW 7: WATCH LATER (Separated Local RustTube Queue vs Experimental YouTube Cookie Queue) */}
              {currentPage === 'watch_later' && (
                <div className="space-y-8">
                  {/* Section A: RustTube Local Watch Later */}
                  <div className="space-y-4">
                    <div className="flex items-center justify-between">
                      <div>
                        <h1 className="text-lg font-bold text-white">
                          A. RustTube Watch Later — Local Persistent Queue ({localWatchLater.length})
                        </h1>
                        <p className="text-xs text-[#7b8094]">
                          Stored locally in SQLite with duplicate prevention. Preserved across restarts and updates.
                        </p>
                      </div>
                      {localWatchLater.length > 0 && (
                        <button
                          type="button"
                          onClick={() =>
                            handleSelectVideo(localWatchLater[0], localWatchLater, 0)
                          }
                          className="flex items-center gap-1.5 px-3.5 py-1.5 rounded-lg bg-[#ff0033] text-white text-xs font-semibold"
                        >
                          <Play className="w-3.5 h-3.5 fill-white" />
                          <span>Play Local Queue</span>
                        </button>
                      )}
                    </div>

                    {localWatchLater.length === 0 ? (
                      <div className="py-12 text-center bg-[#12141c] border border-[#1e2130] rounded-xl text-xs text-[#8a90a6]">
                        Your local RustTube Watch Later queue is empty. Click the bookmark icon on any video to save it.
                      </div>
                    ) : (
                      <div className="grid grid-cols-1 md:grid-cols-2 lg:grid-cols-3 xl:grid-cols-4 gap-4">
                        {localWatchLater.map((v, i) =>
                          renderVideoCard(v, i, localWatchLater)
                        )}
                      </div>
                    )}
                  </div>

                  {/* Section B: YouTube Watch Later (Experimental Cookie Explanation & Opt-In) */}
                  <div className="p-5 rounded-xl bg-[#12141c] border border-[#1e2130] space-y-4">
                    <div className="flex flex-wrap items-start justify-between gap-4">
                      <div className="space-y-1 max-w-2xl">
                        <h2 className="text-sm font-bold text-white">
                          B. YouTube Watch Later (Unofficial Browser Cookie Integration)
                        </h2>
                        <p className="text-xs text-[#8a90a6] leading-relaxed">
                          API Limitation Notice: YouTube&apos;s official Data API v3 disables programmatic access to the personal{' '}
                          <code className="font-mono text-[#d0d4e6]">WL</code> playlist. RustTube does not falsely claim official API sync. You can optionally extract items via{' '}
                          <code className="font-mono text-[#d0d4e6]">yt-dlp --cookies-from-browser</code> into a separate read-only list below without ever overwriting your local RustTube queue.
                        </p>
                      </div>

                      <button
                        type="button"
                        disabled={isMutating}
                        onClick={handleExperimentalWatchLaterImport}
                        className="px-3.5 py-2 rounded-lg bg-[#181b28] hover:bg-[#222638] border border-[#2c3146] text-xs font-semibold text-white whitespace-nowrap shrink-0"
                      >
                        Import via Browser Cookies (Experimental)
                      </button>
                    </div>

                    {experimentalWatchLater.length > 0 && (
                      <div className="grid grid-cols-1 md:grid-cols-2 lg:grid-cols-3 xl:grid-cols-4 gap-4 pt-2">
                        {experimentalWatchLater.map((v, i) =>
                          renderVideoCard(v, i, experimentalWatchLater)
                        )}
                      </div>
                    )}
                  </div>
                </div>
              )}

              {/* VIEW 8: WATCH HISTORY (Search, Remove Single Entry, Clear All, Resume Playback) */}
              {currentPage === 'history' && (
                <div className="space-y-5">
                  <div className="flex flex-wrap items-center justify-between gap-4">
                    <div>
                      <h1 className="text-lg font-bold text-white">
                        Local Watch History ({historyList.length})
                      </h1>
                      <p className="text-xs text-[#7b8094]">
                        Stored privately on your machine with accurate timestamps and resume positions (YouTube&apos;s official Data API does not expose personal watch history).
                      </p>
                    </div>

                    <div className="flex items-center gap-2.5">
                      <div className="relative">
                        <Search className="w-3.5 h-3.5 text-[#666a7e] absolute left-3 top-1/2 -translate-y-1/2" />
                        <input
                          type="text"
                          value={historySearch}
                          onChange={(e) => {
                            setHistorySearch(e.target.value);
                            fetchHistory(e.target.value);
                          }}
                          placeholder="Search watch history..."
                          className="bg-[#151722] border border-[#252838] rounded-lg pl-8 pr-3 py-1.5 text-xs text-white placeholder-[#666a7e] focus:outline-none focus:border-[#ff0033]"
                        />
                      </div>
                      {historyList.length > 0 && (
                        <button
                          type="button"
                          onClick={handleClearHistory}
                          className="px-3 py-1.5 rounded-lg bg-red-950/40 hover:bg-red-900/50 border border-red-800/40 text-xs font-medium text-red-300 whitespace-nowrap"
                        >
                          Clear All History
                        </button>
                      )}
                    </div>
                  </div>

                  {historyList.length === 0 ? (
                    <div className="py-16 text-center bg-[#12141c] border border-[#1e2130] rounded-xl text-xs text-[#8a90a6]">
                      No matching entries in your local watch history.
                    </div>
                  ) : (
                    <div className="space-y-2.5">
                      {historyList.map((item) => (
                        <div
                          key={`hist-${item.id}`}
                          onClick={() =>
                            handleSelectVideo(item, undefined, item.lastPositionSeconds)
                          }
                          className="group cursor-pointer p-3 rounded-xl bg-[#12141c] border border-[#1e2130] hover:border-[#32374f] flex items-center justify-between gap-4"
                        >
                          <div className="flex items-center gap-3.5 min-w-0">
                            <div className="relative w-32 aspect-video rounded-lg bg-[#181a24] overflow-hidden shrink-0">
                              <img
                                src={item.thumbnail}
                                alt={item.title}
                                referrerPolicy="no-referrer"
                                onError={(e) => {
                                  (e.currentTarget as HTMLImageElement).src =
                                    '/assets/aistudio/image_9995d559.png';
                                }}
                                className="w-full h-full object-cover"
                              />
                              <span className="absolute bottom-1 right-1 bg-black/85 text-white text-[10px] font-mono tabular-nums px-1 rounded">
                                {item.duration}
                              </span>
                            </div>

                            <div className="min-w-0 space-y-1">
                              <h3 className="text-xs font-semibold text-white group-hover:text-[#ff4d6d] truncate">
                                {item.title}
                              </h3>
                              <div className="flex flex-wrap items-center gap-2 text-[11px] text-[#8a90a6] font-mono tabular-nums">
                                <span className="font-sans">{item.channel}</span>
                                {item.watchedAt && (
                                  <>
                                    <span aria-hidden="true">·</span>
                                    <span>
                                      Watched {new Date(item.watchedAt).toLocaleString()}
                                    </span>
                                  </>
                                )}
                                {typeof item.lastPositionSeconds === 'number' &&
                                  item.lastPositionSeconds > 0 && (
                                    <>
                                      <span aria-hidden="true">·</span>
                                      <span className="text-[#ff4d6d]">
                                        Continue from {formatSeconds(item.lastPositionSeconds)}
                                      </span>
                                    </>
                                  )}
                              </div>
                            </div>
                          </div>

                          <button
                            type="button"
                            onClick={(e) => handleRemoveHistoryEntry(item.id, e)}
                            className="p-2 rounded-lg bg-[#181b26] text-[#8a90a6] hover:text-red-400 shrink-0"
                            title="Remove Entry from History"
                          >
                            <Trash2 className="w-3.5 h-3.5" />
                          </button>
                        </div>
                      ))}
                    </div>
                  )}
                </div>
              )}

              {/* VIEW 9: SEARCH RESULTS */}
              {currentPage === 'search' && (
                <div className="space-y-5">
                  <div className="flex items-center justify-between">
                    <div>
                      <h1 className="text-lg font-bold text-white">
                        Search Results for &ldquo;{activeSearchTerm}&rdquo;
                      </h1>
                      <p className="text-xs text-[#7b8094] font-mono tabular-nums">
                        {searchVideos.length} results resolved via yt-dlp
                      </p>
                    </div>
                  </div>

                  {isLoading ? (
                    <div className="py-24 flex flex-col items-center justify-center gap-3">
                      <Loader2 className="w-7 h-7 text-[#ff0033] animate-spin" />
                      <p className="text-xs text-[#8c91a6]">Searching YouTube via yt-dlp...</p>
                    </div>
                  ) : (
                    <div className="grid grid-cols-1 md:grid-cols-2 lg:grid-cols-3 xl:grid-cols-4 gap-4">
                      {searchVideos.map((v, i) => renderVideoCard(v, i, searchVideos))}
                    </div>
                  )}
                </div>
              )}

              {/* VIEW 10: DEDICATED WATCH PAGE */}
              {currentPage === 'watch' && selectedVideo && (
                <div className="max-w-6xl mx-auto space-y-5">
                  <div className="flex flex-wrap items-center justify-between gap-3">
                    <div className="flex items-center gap-2">
                      <button
                        type="button"
                        onClick={navigateBack}
                        className="flex items-center gap-1.5 px-3 py-1.5 rounded-lg bg-[#141620] hover:bg-[#1a1d2c] border border-[#232636] text-xs text-[#c8cde0] hover:text-white"
                      >
                        <ArrowLeft className="w-3.5 h-3.5" />
                        <span>Back</span>
                      </button>
                      {playbackQueue.length > 1 && (
                        <button
                          type="button"
                          onClick={handlePlayNextInQueue}
                          className="flex items-center gap-1.5 px-3 py-1.5 rounded-lg bg-[#141620] hover:bg-[#1a1d2c] border border-[#232636] text-xs text-[#c8cde0] hover:text-white"
                        >
                          <SkipForward className="w-3.5 h-3.5" />
                          <span>Next in Queue</span>
                        </button>
                      )}
                    </div>

                    <div className="flex items-center gap-2 text-xs font-mono tabular-nums">
                      <span className="text-[#8a90a6]">
                        Position: {formatSeconds(currentTime)}
                      </span>
                      <span>·</span>
                      <select
                        value={quality}
                        onChange={(e) => setQuality(e.target.value)}
                        className="bg-[#151722] border border-[#252838] rounded px-2 py-1 text-xs text-white"
                      >
                        {(
                          videoDetail?.availableQualities || [
                            '1080p',
                            '720p',
                            '480p',
                            '360p',
                          ]
                        ).map((q) => (
                          <option key={q} value={q}>
                            {q}
                          </option>
                        ))}
                      </select>
                      <select
                        value={speed}
                        onChange={(e) => setSpeed( parseFloat(e.target.value) )}
                        className="bg-[#151722] border border-[#252838] rounded px-2 py-1 text-xs text-white"
                      >
                        {[0.5, 0.75, 1.0, 1.25, 1.5, 2.0].map((s) => (
                          <option key={s} value={s}>
                            {s}x
                          </option>
                        ))}
                      </select>
                    </div>
                  </div>

                  {/* Video Player Canvas */}
                  <div className="relative aspect-video w-full rounded-xl overflow-hidden bg-black border border-[#1f212d]">
                    <iframe
                      src={`${selectedVideo.embedUrl}${
                        currentTime > 5 ? `&start=${currentTime}` : ''
                      }`}
                      title={selectedVideo.title}
                      className="w-full h-full"
                      allow="accelerometer; autoplay; clipboard-write; encrypted-media; gyroscope; picture-in-picture"
                      allowFullScreen
                    />
                  </div>

                  {/* Metadata & Interactive Controls */}
                  <div className="space-y-3">
                    <h1 className="text-xl font-bold text-white">{selectedVideo.title}</h1>

                    <div className="flex flex-wrap items-center justify-between gap-4 pb-3 border-b border-[#1c1e29]">
                      <div
                        onClick={() =>
                          handleOpenChannel(selectedVideo.channelId || selectedVideo.channel)
                        }
                        className="flex items-center gap-3 cursor-pointer group"
                      >
                        <div className="w-10 h-10 rounded-full bg-[#ff0033] flex items-center justify-center font-bold text-white text-sm shrink-0">
                          {selectedVideo.channel.slice(0, 1).toUpperCase()}
                        </div>
                        <div>
                          <p className="text-sm font-semibold text-white group-hover:text-[#ff4d6d]">
                            {selectedVideo.channel}
                          </p>
                          <p className="text-[11px] text-[#7b8094] font-mono tabular-nums">
                            {selectedVideo.views} · {selectedVideo.published || 'Recent'}
                          </p>
                        </div>
                      </div>

                      <div className="flex flex-wrap items-center gap-2">
                        <button
                          type="button"
                          disabled={isMutating}
                          onClick={() => handleToggleLike(selectedVideo)}
                          className={`flex items-center gap-1.5 px-3.5 py-1.5 rounded-lg text-xs font-medium border transition-colors ${
                            likedVideos.some((v) => v.id === selectedVideo.id)
                              ? 'bg-[#ff0033]/20 border-[#ff0033]/50 text-[#ff4d6d]'
                              : 'bg-[#151620] border-[#222533] text-[#c8cde0] hover:text-white'
                          }`}
                        >
                          <ThumbsUp className="w-3.5 h-3.5" />
                          <span>
                            {likedVideos.some((v) => v.id === selectedVideo.id)
                              ? 'Liked'
                              : 'Like'}
                          </span>
                        </button>

                        <button
                          type="button"
                          onClick={(e) => handleToggleWatchLater(selectedVideo, e)}
                          className={`flex items-center gap-1.5 px-3.5 py-1.5 rounded-lg text-xs font-medium border transition-colors ${
                            localWatchLater.some((v) => v.id === selectedVideo.id)
                              ? 'bg-[#ff0033]/20 border-[#ff0033]/50 text-[#ff4d6d]'
                              : 'bg-[#151620] border-[#222533] text-[#c8cde0] hover:text-white'
                          }`}
                        >
                          {localWatchLater.some((v) => v.id === selectedVideo.id) ? (
                            <>
                              <BookmarkCheck className="w-3.5 h-3.5" />
                              <span>In Local Watch Later</span>
                            </>
                          ) : (
                            <>
                              <Bookmark className="w-3.5 h-3.5" />
                              <span>Save to Watch Later</span>
                            </>
                          )}
                        </button>

                        {playlists.length > 0 && (
                          <select
                            defaultValue=""
                            onChange={(e) => {
                              if (e.target.value) {
                                handleAddVideoToPlaylist(e.target.value, selectedVideo);
                                e.target.value = '';
                              }
                            }}
                            className="bg-[#151620] border border-[#222533] rounded-lg px-3 py-1.5 text-xs text-[#c8cde0] hover:text-white focus:outline-none"
                          >
                            <option value="" disabled>
                              + Add to Playlist...
                            </option>
                            {playlists.map((pl) => (
                              <option key={pl.playlistId} value={pl.playlistId}>
                                {pl.title}
                              </option>
                            ))}
                          </select>
                        )}
                      </div>
                    </div>

                    <div className="p-4 rounded-xl bg-[#12141c] border border-[#1e202d] text-xs text-[#b0b4c8] leading-relaxed whitespace-pre-line max-h-48 overflow-y-auto">
                      {videoDetail?.description ||
                        selectedVideo.description ||
                        'No description available.'}
                    </div>
                  </div>

                  {/* Up Next / Related Videos */}
                  <div className="pt-4 space-y-3">
                    <h3 className="text-sm font-bold text-white">
                      {playbackQueue.length > 1 ? 'Playlist Queue' : 'Related Videos'}
                    </h3>
                    <div className="grid grid-cols-1 md:grid-cols-2 lg:grid-cols-4 gap-4">
                      {(playbackQueue.length > 1 ? playbackQueue : homeVideos)
                        .filter((v) => v.id !== selectedVideo.id)
                        .slice(0, 8)
                        .map((item, idx) => renderVideoCard(item, idx, playbackQueue))}
                    </div>
                  </div>
                </div>
              )}

              {/* VIEW 11: SETTINGS, ACCOUNT SYNC & NATIVE APPLICATION UPDATER */}
              {currentPage === 'settings' && (
                <div className="max-w-3xl mx-auto space-y-6">
                  <div>
                    <h1 className="text-xl font-bold text-white">
                      Application Settings, Account Sync & Native Updater
                    </h1>
                    <p className="text-xs text-[#7b8094]">
                      Persistent configuration stored in <code className="font-mono">%APPDATA%\RustTube\config.toml</code> and <code className="font-mono">%LOCALAPPDATA%\RustTube\data\rusttube.db</code>.
                    </p>
                  </div>

                  {/* 1. Google OAuth2 PKCE & Account Synchronization */}
                  <div className="p-5 rounded-xl bg-[#12141c] border border-[#1e2130] space-y-4">
                    <div className="flex items-center justify-between">
                      <div>
                        <h2 className="text-sm font-bold text-white">
                          Google OAuth2 PKCE & Account Synchronization
                        </h2>
                        <p className="text-xs text-[#8a90a6]">
                          Refresh tokens are persisted in Windows Credential Manager (<code className="font-mono">RustTube_Google_OAuth</code>) and never written to logs.
                        </p>
                      </div>
                      <span className="text-xs font-mono tabular-nums text-[#9ea3b8]">
                        {auth.isLoggedIn ? 'Session Active' : 'Signed Out'}
                      </span>
                    </div>

                    {auth.isLoggedIn ? (
                      <div className="p-3.5 rounded-lg bg-[#171924] border border-[#252838] flex flex-wrap items-center justify-between gap-4">
                        <div className="flex items-center gap-3">
                          <img
                            src={auth.avatarUrl || '/assets/aistudio/image_9995d559.png'}
                            alt={auth.userName}
                            referrerPolicy="no-referrer"
                            className="w-10 h-10 rounded-full object-cover"
                          />
                          <div>
                            <p className="text-xs font-semibold text-white">{auth.userName}</p>
                            <p className="text-[11px] text-[#8a90a6] font-mono">
                              {auth.email} · {auth.handle}
                            </p>
                          </div>
                        </div>

                        <div className="flex items-center gap-2">
                          <button
                            type="button"
                            disabled={isSyncing}
                            onClick={handleManualSync}
                            className="flex items-center gap-1.5 px-3 py-1.5 rounded-md bg-[#ff0033] hover:bg-[#d9002c] text-white text-xs font-semibold"
                          >
                            <RefreshCw className={`w-3.5 h-3.5 ${isSyncing ? 'animate-spin' : ''}`} />
                            <span>Sync Account Now</span>
                          </button>
                          <button
                            type="button"
                            onClick={handleLogout}
                            className="flex items-center gap-1.5 px-3 py-1.5 rounded-md bg-red-950/40 hover:bg-red-900/50 border border-red-800/40 text-red-300 text-xs font-medium"
                          >
                            <LogOut className="w-3.5 h-3.5" />
                            <span>Sign Out</span>
                          </button>
                        </div>
                      </div>
                    ) : (
                      <div className="space-y-3">
                        <div className="flex flex-wrap items-center gap-2">
                          <input
                            type="password"
                            value={oauthAccessTokenInput}
                            onChange={(e) => setOauthAccessTokenInput(e.target.value)}
                            placeholder="Optional: Paste live Google OAuth2 Bearer Token for direct API verification..."
                            className="flex-1 bg-[#171924] border border-[#262a3b] rounded-lg px-3 py-2 text-xs text-white placeholder-[#666a7e]"
                          />
                          <button
                            type="button"
                            onClick={handleLogin}
                            className="px-4 py-2 bg-[#ff0033] hover:bg-[#d9002c] text-white text-xs font-semibold rounded-lg"
                          >
                            Sign In with Google
                          </button>
                        </div>
                      </div>
                    )}

                    <div className="grid grid-cols-1 sm:grid-cols-3 gap-3 text-xs font-mono tabular-nums pt-1">
                      <div className="p-3 rounded-lg bg-[#161822] border border-[#222534]">
                        <div className="text-[#7b8094]">Synced Subscriptions</div>
                        <div className="text-base font-bold text-white mt-0.5">
                          {subscriptions.length}
                        </div>
                      </div>
                      <div className="p-3 rounded-lg bg-[#161822] border border-[#222534]">
                        <div className="text-[#7b8094]">Synced Playlists</div>
                        <div className="text-base font-bold text-white mt-0.5">
                          {playlists.length}
                        </div>
                      </div>
                      <div className="p-3 rounded-lg bg-[#161822] border border-[#222534]">
                        <div className="text-[#7b8094]">Synced Liked Videos</div>
                        <div className="text-base font-bold text-white mt-0.5">
                          {likedVideos.length}
                        </div>
                      </div>
                    </div>
                  </div>

                  {/* 2. Native Windows Application Updater (Phase 4) */}
                  <div className="p-5 rounded-xl bg-[#12141c] border border-[#1e2130] space-y-4">
                    <div className="flex flex-wrap items-center justify-between gap-4">
                      <div>
                        <h2 className="text-sm font-bold text-white flex items-center gap-2">
                          <ShieldCheck className="w-4 h-4 text-[#ff4d6d]" />
                          <span>Native Windows Application Updater (GitHub Releases)</span>
                        </h2>
                        <p className="text-xs text-[#8a90a6]">
                          Repository:{' '}
                          <a
                            href="https://github.com/duongducdungnguyen-dot/RustTube"
                            target="_blank"
                            rel="noreferrer"
                            className="text-[#ff4d6d] hover:underline font-mono"
                          >
                            duongducdungnguyen-dot/RustTube
                          </a>
                        </p>
                      </div>

                      <div className="flex items-center gap-2">
                        <button
                          type="button"
                          disabled={updater.status === 'checking'}
                          onClick={handleCheckUpdates}
                          className="flex items-center gap-1.5 px-3.5 py-1.5 rounded-lg bg-[#181b28] hover:bg-[#222638] border border-[#2c3146] text-xs font-semibold text-white"
                        >
                          <RefreshCw
                            className={`w-3.5 h-3.5 ${
                              updater.status === 'checking' ? 'animate-spin' : ''
                            }`}
                          />
                          <span>Check for Updates</span>
                        </button>
                      </div>
                    </div>

                    <div className="grid grid-cols-1 sm:grid-cols-3 gap-3 text-xs font-mono tabular-nums">
                      <div className="p-3 rounded-lg bg-[#161822] border border-[#222534]">
                        <div className="text-[#7b8094]">Installed Version</div>
                        <div className="text-sm font-bold text-white mt-0.5">
                          v{updater.currentVersion}
                        </div>
                      </div>
                      <div className="p-3 rounded-lg bg-[#161822] border border-[#222534]">
                        <div className="text-[#7b8094]">Latest GitHub Release</div>
                        <div className="text-sm font-bold text-white mt-0.5">
                          v{updater.latestRelease?.version || updater.currentVersion}
                        </div>
                      </div>
                      <div className="p-3 rounded-lg bg-[#161822] border border-[#222534]">
                        <div className="text-[#7b8094]">Last Checked</div>
                        <div className="text-sm font-bold text-white mt-0.5">
                          {updater.lastCheckedAt
                            ? new Date(updater.lastCheckedAt).toLocaleTimeString()
                            : 'On Startup'}
                        </div>
                      </div>
                    </div>

                    {updater.latestRelease && (
                      <div className="p-3.5 rounded-lg bg-[#161822] border border-[#242838] space-y-2 text-xs">
                        <div className="flex items-center justify-between font-mono tabular-nums text-[#9ea3b8]">
                          <span>Asset: {updater.latestRelease.assetName}</span>
                          <span>
                            SHA-256: {updater.latestRelease.sha256?.slice(0, 16)}...
                          </span>
                        </div>
                        <p className="text-[#c8cde0]">{updater.latestRelease.releaseNotes}</p>
                        <div className="pt-2 flex flex-wrap items-center gap-2.5">
                          <button
                            type="button"
                            disabled={isMutating}
                            onClick={handleStageUpdate}
                            className="flex items-center gap-1.5 px-3 py-1.5 rounded-md bg-[#1f2334] hover:bg-[#282d42] border border-[#323852] text-xs font-semibold text-white"
                          >
                            <Download className="w-3.5 h-3.5" />
                            <span>1. Download & Verify SHA-256 in Staging</span>
                          </button>
                          {updater.status === 'staged' && (
                            <button
                              type="button"
                              disabled={isMutating}
                              onClick={handleApplyUpdate}
                              className="flex items-center gap-1.5 px-3.5 py-1.5 rounded-md bg-[#ff0033] hover:bg-[#d9002c] text-xs font-semibold text-white"
                            >
                              <span>2. Launch rusttube-updater.exe & Restart</span>
                            </button>
                          )}
                        </div>
                      </div>
                    )}

                    <div className="flex flex-wrap items-center gap-6 pt-1 text-xs text-[#9ea3b8]">
                      <label className="flex items-center gap-2 cursor-pointer">
                        <input
                          type="checkbox"
                          checked={settings.autoCheckUpdates}
                          onChange={(e) =>
                            handleSaveSettings({ autoCheckUpdates: e.target.checked })
                          }
                          className="rounded accent-[#ff0033]"
                        />
                        <span>Automatically check GitHub Releases on startup</span>
                      </label>
                      <label className="flex items-center gap-2 cursor-pointer">
                        <input
                          type="checkbox"
                          checked={settings.includePrereleases}
                          onChange={(e) =>
                            handleSaveSettings({ includePrereleases: e.target.checked })
                          }
                          className="rounded accent-[#ff0033]"
                        />
                        <span>Include prereleases</span>
                      </label>
                    </div>
                  </div>

                  {/* 3. Playback, Hardware Decoding & Experimental Cookie Providers */}
                  <div className="p-5 rounded-xl bg-[#12141c] border border-[#1e2130] space-y-4">
                    <h2 className="text-sm font-bold text-white">
                      Playback (libmpv) & Experimental Cookie Providers (yt-dlp)
                    </h2>
                    <div className="grid grid-cols-1 sm:grid-cols-2 gap-4 text-xs">
                      <div className="space-y-1.5">
                        <label className="text-[#9ea3b8] block">
                          Browser Cookies for yt-dlp (--cookies-from-browser)
                        </label>
                        <select
                          value={settings.cookiesBrowser}
                          onChange={(e) =>
                            handleSaveSettings({
                              cookiesBrowser: e.target.value as AppSettings['cookiesBrowser'],
                            })
                          }
                          className="w-full bg-[#171924] border border-[#272b3d] rounded-lg px-3 py-2 text-white"
                        >
                          <option value="none">None (Public Streams & Official API Only)</option>
                          <option value="chrome">Google Chrome</option>
                          <option value="edge">Microsoft Edge</option>
                          <option value="firefox">Mozilla Firefox</option>
                        </select>
                      </div>

                      <div className="space-y-1.5">
                        <label className="text-[#9ea3b8] block">
                          GPU Hardware Decoding (libmpv hwdec)
                        </label>
                        <select
                          value={settings.hwdec}
                          onChange={(e) =>
                            handleSaveSettings({
                              hwdec: e.target.value as AppSettings['hwdec'],
                            })
                          }
                          className="w-full bg-[#171924] border border-[#272b3d] rounded-lg px-3 py-2 text-white"
                        >
                          <option value="auto-safe">auto-safe (Recommended)</option>
                          <option value="d3d11va">d3d11va (Windows DirectX 11)</option>
                          <option value="nvdec">nvdec (NVIDIA RTX)</option>
                          <option value="software">software</option>
                        </select>
                      </div>
                    </div>

                    <div className="space-y-2 pt-2 text-xs text-[#9ea3b8]">
                      <label className="flex items-center gap-2 cursor-pointer">
                        <input
                          type="checkbox"
                          checked={settings.experimentalCookieHomeFeed}
                          onChange={(e) =>
                            handleSaveSettings({
                              experimentalCookieHomeFeed: e.target.checked,
                            })
                          }
                          className="rounded accent-[#ff0033]"
                        />
                        <span>
                          Consent to optional experimental cookie-based YouTube homepage provider
                        </span>
                      </label>
                      <label className="flex items-center gap-2 cursor-pointer">
                        <input
                          type="checkbox"
                          checked={settings.experimentalCookieWatchLater}
                          onChange={(e) =>
                            handleSaveSettings({
                              experimentalCookieWatchLater: e.target.checked,
                            })
                          }
                          className="rounded accent-[#ff0033]"
                        />
                        <span>
                          Consent to optional experimental cookie-based YouTube Watch Later import
                        </span>
                      </label>
                    </div>
                  </div>
                </div>
              )}
            </div>

            {/* Docked Mini-Player Bar when browsing outside the Watch page */}
            {selectedVideo && currentPage !== 'watch' && (
              <div className="border-t border-[#1e202b] bg-[#101218] px-5 py-2.5 shrink-0 flex items-center justify-between gap-4">
                <div
                  onClick={() => navigateTo('watch')}
                  className="flex items-center gap-3 cursor-pointer group min-w-0"
                >
                  <div className="relative w-14 h-8 rounded bg-[#1a1c26] overflow-hidden shrink-0 border border-[#272a3b]">
                    <img
                      src={selectedVideo.thumbnail}
                      alt={selectedVideo.title}
                      referrerPolicy="no-referrer"
                      className="w-full h-full object-cover"
                    />
                  </div>
                  <div className="min-w-0">
                    <p className="text-xs font-semibold text-white truncate group-hover:text-[#ff4d6d]">
                      {selectedVideo.title}
                    </p>
                    <p className="text-[11px] text-[#7d8296] truncate font-mono tabular-nums">
                      {selectedVideo.channel} · {formatSeconds(currentTime)}
                    </p>
                  </div>
                </div>

                <div className="flex items-center gap-2 shrink-0">
                  <button
                    type="button"
                    onClick={() => setIsPlaying(!isPlaying)}
                    className="p-1.5 rounded-md bg-[#181b26] text-white hover:bg-[#222636]"
                    title={isPlaying ? 'Pause' : 'Play'}
                  >
                    {isPlaying ? <Pause className="w-3.5 h-3.5" /> : <Play className="w-3.5 h-3.5" />}
                  </button>
                  <button
                    type="button"
                    onClick={() => navigateTo('watch')}
                    className="px-3 py-1.5 bg-[#1a1c26] hover:bg-[#222533] text-xs text-white rounded-md border border-[#2b2e40] font-medium"
                  >
                    Return to Watch View
                  </button>
                  <button
                    type="button"
                    onClick={() => setSelectedVideo(null)}
                    className="p-1.5 text-[#7b8094] hover:text-white rounded-md"
                    title="Close Player"
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
