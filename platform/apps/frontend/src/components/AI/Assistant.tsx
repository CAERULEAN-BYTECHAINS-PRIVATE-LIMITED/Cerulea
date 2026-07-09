'use client';

import React, { useEffect, useRef, useState, useMemo } from 'react';
import {
  Fab, Drawer, Box, Typography, TextField, IconButton, Stack,
  Chip, Divider, Avatar, Paper, Tooltip,
} from '@mui/material';
import { alpha, useTheme } from '@mui/material/styles';
import { motion, AnimatePresence } from 'framer-motion';
import SmartToyIcon from '@mui/icons-material/SmartToy';
import CloseIcon from '@mui/icons-material/Close';
import SendIcon from '@mui/icons-material/Send';
import AutoAwesomeIcon from '@mui/icons-material/AutoAwesome';
import { usePathname } from 'next/navigation';
import { useStudio } from '@/context/StudioContext';
import { api } from '@/lib/apiClient';

const fabVariants = {
  hidden: { scale: 0, y: 50, opacity: 0 },
  visible: {
    scale: 1, y: 0, opacity: 1,
    transition: { type: 'spring' as const, stiffness: 260, damping: 20, delay: 0.5 },
  },
};

type ChatRole = 'user' | 'assistant';
type ChatMessage = { id: string; role: ChatRole; text: string; createdAt: Date };

function makeId(prefix = 'msg') {
  return `${prefix}_${Math.random().toString(36).slice(2, 10)}_${Date.now().toString(36)}`;
}

function normalize(s: string) {
  return s.trim().toLowerCase().replace(/[?!.]+$/g, '').replace(/\s+/g, ' ');
}

function formatTime(d: Date) {
  const hh = String(d.getHours()).padStart(2, '0');
  const mm = String(d.getMinutes()).padStart(2, '0');
  return `${hh}:${mm}`;
}

const QUICK_PROMPTS = [
  'What modules should I add?',
  'Explain smart contracts',
  'How does tokenomics work?',
  'What is an entity?',
];

export default function Assistant() {
  const pathname = usePathname();
  const studio = useStudio();
  const theme = useTheme();

  const [open, setOpen] = useState(false);
  const [input, setInput] = useState('');
  const [messages, setMessages] = useState<ChatMessage[]>(() => [
    {
      id: makeId('assistant'),
      role: 'assistant',
      text: "Hi! I'm Cerulea AI. I can help you design your app, choose modules, configure your tokenomics, or answer any question about the Studio.\n\nWhat would you like to build?",
      createdAt: new Date(),
    },
  ]);
  const [isTyping, setIsTyping] = useState(false);
  const scrollRef = useRef<HTMLDivElement | null>(null);
  const typingTimerRef = useRef<number | null>(null);
  const streamTimerRef = useRef<number | null>(null);

  useEffect(() => {
    if (scrollRef.current) {
      scrollRef.current.scrollTo({ top: scrollRef.current.scrollHeight, behavior: 'smooth' });
    }
  }, [messages, isTyping]);

  useEffect(() => {
    return () => {
      if (typingTimerRef.current) window.clearTimeout(typingTimerRef.current);
      if (streamTimerRef.current) window.clearInterval(streamTimerRef.current);
    };
  }, []);

  function streamAssistantMessage(fullText: string) {
    const msgId = makeId('assistant');
    const createdAt = new Date();
    setMessages((prev) => [...prev, { id: msgId, role: 'assistant', text: '', createdAt }]);
    let i = 0;
    streamTimerRef.current = window.setInterval(() => {
      const chunk = fullText.slice(i, i + (Math.random() < 0.85 ? 1 : 2));
      i += chunk.length;
      setMessages((prev) => prev.map((m) => m.id === msgId ? { ...m, text: m.text + chunk } : m));
      if (i >= fullText.length) {
        window.clearInterval(streamTimerRef.current!);
        streamTimerRef.current = null;
        setIsTyping(false);
      }
    }, 16 + Math.floor(Math.random() * 12));
  }

  function buildStudioSnapshot() {
    return {
      currentRoute: pathname,
      studioState: {
        projectId: studio.projectId,
        projectType: studio.projectType,
        templateId: studio.templateId,
        selectedModules: studio.selectedModules,
        appMetadata: studio.appMetadata,
        appGoal: studio.appGoal,
      },
    };
  }

  function readProjectMemory() {
    try {
      const raw = localStorage.getItem(`ceruleai:memory:${studio.projectId || 'local'}`);
      return raw ? JSON.parse(raw) : {};
    } catch { return {}; }
  }

  function buildProjectMemory() {
    const prev = readProjectMemory();
    const next = { ...prev, projectType: studio.projectType, selectedModules: studio.selectedModules, appMetadata: studio.appMetadata, updatedAt: new Date().toISOString() };
    try { localStorage.setItem(`ceruleai:memory:${studio.projectId || 'local'}`, JSON.stringify(next)); } catch {}
    return next;
  }

  function toHistoryPayload(msgs: ChatMessage[]) {
    const budget = 12000;
    const out: { role: ChatRole; text: string }[] = [];
    let used = 0;
    for (let i = msgs.length - 1; i >= 0; i--) {
      const line = `${msgs[i].role}:${msgs[i].text}\n`;
      if (used + line.length > budget) break;
      out.unshift({ role: msgs[i].role, text: msgs[i].text });
      used += line.length;
    }
    return out;
  }

  async function handleSend(text?: string) {
    const msg = (text || input).trim();
    if (!msg) return;
    setInput('');
    const userMsg: ChatMessage = { id: makeId('user'), role: 'user', text: msg, createdAt: new Date() };
    setMessages((prev) => [...prev, userMsg]);
    setIsTyping(true);
    typingTimerRef.current = window.setTimeout(async () => {
      try {
        const res = await api<{ reply: string }>(`/api/ceruleai`, {
          method: 'POST',
          body: JSON.stringify({
            message: msg,
            history: toHistoryPayload(messages),
            studioSnapshot: buildStudioSnapshot(),
            projectMemory: buildProjectMemory(),
          }),
        });
        streamAssistantMessage(res.reply || "I couldn't generate a response right now.");
      } catch (e: any) {
        setIsTyping(false);
        streamAssistantMessage(typeof e?.message === 'string' ? `Something went wrong: ${e.message}` : 'Something went wrong generating the response.');
      }
    }, 400 + Math.floor(Math.random() * 300));
  }

  const isLight = theme.palette.mode === 'light';

  return (
    <>
      <AnimatePresence>
        {!open && (
          <motion.div
            variants={fabVariants}
            initial="hidden"
            animate="visible"
            exit="hidden"
            style={{ position: 'fixed', bottom: 32, right: 32, zIndex: 1301, display: 'flex', flexDirection: 'column', alignItems: 'center', gap: 4 }}
          >
            <Fab
              color="primary"
              aria-label="Cerulea AI"
              onClick={() => setOpen(true)}
              sx={{
                background: 'linear-gradient(135deg, #3d5afe 0%, #7c3aed 100%)',
                boxShadow: `0 8px 32px ${alpha('#3d5afe', 0.45)}`,
                '&:hover': { boxShadow: `0 12px 40px ${alpha('#3d5afe', 0.55)}` },
              }}
            >
              <AutoAwesomeIcon />
            </Fab>
            <Typography
              variant="caption"
              sx={{ fontWeight: 800, fontSize: '0.62rem', letterSpacing: 0.5, color: 'text.secondary', textAlign: 'center', lineHeight: 1, userSelect: 'none' }}
            >
              Cerulea AI
            </Typography>
          </motion.div>
        )}
      </AnimatePresence>

      <Drawer
        anchor="right"
        open={open}
        onClose={() => setOpen(false)}
        PaperProps={{
          sx: {
            width: { xs: '92%', sm: 420 },
            border: 'none',
            bgcolor: isLight ? '#ffffff' : '#131823',
            display: 'flex',
            flexDirection: 'column',
          },
        }}
      >
        {/* Header */}
        <Box
          sx={{
            px: 3, pt: 2.5, pb: 2,
            background: 'linear-gradient(135deg, #3d5afe 0%, #7c3aed 100%)',
            flexShrink: 0,
          }}
        >
          <Stack direction="row" alignItems="center" justifyContent="space-between">
            <Stack direction="row" alignItems="center" spacing={1.5}>
              <Box
                sx={{
                  width: 36, height: 36, borderRadius: '50%',
                  bgcolor: 'rgba(255,255,255,0.2)',
                  display: 'flex', alignItems: 'center', justifyContent: 'center',
                }}
              >
                <AutoAwesomeIcon sx={{ fontSize: 18, color: 'white' }} />
              </Box>
              <Box>
                <Typography variant="subtitle1" fontWeight={800} sx={{ color: 'white', lineHeight: 1.2 }}>
                  Cerulea AI
                </Typography>
                <Stack direction="row" alignItems="center" spacing={0.5}>
                  <Box sx={{ width: 6, height: 6, borderRadius: '50%', bgcolor: '#4ade80' }} />
                  <Typography variant="caption" sx={{ color: 'rgba(255,255,255,0.75)', fontWeight: 600 }}>
                    Online
                  </Typography>
                </Stack>
              </Box>
            </Stack>
            <IconButton
              onClick={() => setOpen(false)}
              size="small"
              sx={{ color: 'rgba(255,255,255,0.8)', '&:hover': { bgcolor: 'rgba(255,255,255,0.15)' } }}
            >
              <CloseIcon fontSize="small" />
            </IconButton>
          </Stack>

          {/* Context chip */}
          {studio.projectType && (
            <Chip
              label={studio.projectType === 'blockchain' ? 'Private Blockchain project' : 'dApp project'}
              size="small"
              sx={{
                mt: 1.5, bgcolor: 'rgba(255,255,255,0.18)', color: 'white',
                fontWeight: 600, fontSize: '0.68rem', border: '1px solid rgba(255,255,255,0.25)',
              }}
            />
          )}
        </Box>

        {/* Messages */}
        <Box
          ref={scrollRef}
          sx={{ flex: 1, overflowY: 'auto', px: 2.5, py: 2, display: 'flex', flexDirection: 'column', gap: 1.5 }}
        >
          {messages.map((m) => {
            const isUser = m.role === 'user';
            return (
              <Box key={m.id} sx={{ display: 'flex', flexDirection: isUser ? 'row-reverse' : 'row', alignItems: 'flex-end', gap: 1 }}>
                {!isUser && (
                  <Avatar
                    sx={{
                      width: 28, height: 28, flexShrink: 0, mb: 0.5,
                      background: 'linear-gradient(135deg, #3d5afe 0%, #7c3aed 100%)',
                      fontSize: '0.75rem',
                    }}
                  >
                    <AutoAwesomeIcon sx={{ fontSize: 14 }} />
                  </Avatar>
                )}
                <Box sx={{ maxWidth: '82%' }}>
                  <Paper
                    elevation={0}
                    sx={{
                      px: 1.75, py: 1.25, borderRadius: isUser ? '18px 18px 4px 18px' : '4px 18px 18px 18px',
                      bgcolor: isUser
                        ? 'primary.main'
                        : isLight ? '#f1f5ff' : 'rgba(255,255,255,0.07)',
                      border: `1px solid ${isUser ? 'transparent' : alpha(theme.palette.divider, 0.5)}`,
                    }}
                  >
                    <Typography
                      variant="body2"
                      sx={{
                        lineHeight: 1.6, whiteSpace: 'pre-line',
                        color: isUser ? 'white' : 'text.primary',
                        fontSize: '0.84rem',
                      }}
                    >
                      {m.text}
                    </Typography>
                  </Paper>
                  <Typography
                    variant="caption"
                    sx={{
                      opacity: 0.45, display: 'block',
                      mt: 0.4, fontSize: '0.6rem', fontWeight: 600,
                      textAlign: isUser ? 'right' : 'left',
                      mr: isUser ? 0.5 : 0, ml: isUser ? 0 : 0.5,
                    }}
                  >
                    {formatTime(m.createdAt)}
                  </Typography>
                </Box>
              </Box>
            );
          })}

          {isTyping && (
            <Box sx={{ display: 'flex', alignItems: 'flex-end', gap: 1 }}>
              <Avatar
                sx={{
                  width: 28, height: 28, flexShrink: 0, mb: 0.5,
                  background: 'linear-gradient(135deg, #3d5afe 0%, #7c3aed 100%)',
                }}
              >
                <AutoAwesomeIcon sx={{ fontSize: 14 }} />
              </Avatar>
              <Paper
                elevation={0}
                sx={{
                  px: 2, py: 1.25, borderRadius: '4px 18px 18px 18px',
                  bgcolor: isLight ? '#f1f5ff' : 'rgba(255,255,255,0.07)',
                  border: `1px solid ${alpha(theme.palette.divider, 0.5)}`,
                  display: 'flex', alignItems: 'center', gap: 0.5,
                }}
              >
                {[0, 0.18, 0.36].map((delay, i) => (
                  <Box
                    key={i}
                    sx={{
                      width: 7, height: 7, borderRadius: '50%',
                      bgcolor: 'primary.main',
                      animation: 'dotBlink 1.1s infinite',
                      animationDelay: `${delay}s`,
                      '@keyframes dotBlink': {
                        '0%': { opacity: 0.3, transform: 'translateY(0px)' },
                        '20%': { opacity: 1, transform: 'translateY(-3px)' },
                        '40%': { opacity: 0.3, transform: 'translateY(0px)' },
                        '100%': { opacity: 0.3 },
                      },
                    }}
                  />
                ))}
              </Paper>
            </Box>
          )}
        </Box>

        {/* Quick prompts */}
        {messages.length <= 2 && !isTyping && (
          <Box sx={{ px: 2.5, pb: 1.5, flexShrink: 0 }}>
            <Typography variant="caption" fontWeight={700} color="text.secondary" sx={{ px: 0.5, mb: 0.75, display: 'block', letterSpacing: 0.5 }}>
              QUICK QUESTIONS
            </Typography>
            <Stack direction="row" flexWrap="wrap" gap={0.75}>
              {QUICK_PROMPTS.map((q) => (
                <Chip
                  key={q}
                  label={q}
                  size="small"
                  variant="outlined"
                  clickable
                  onClick={() => handleSend(q)}
                  sx={{ fontWeight: 600, fontSize: '0.72rem', borderRadius: '999px' }}
                />
              ))}
            </Stack>
          </Box>
        )}

        <Divider />

        {/* Input */}
        <Box
          component="form"
          onSubmit={(e) => { e.preventDefault(); handleSend(); }}
          sx={{
            px: 2, py: 1.75, flexShrink: 0,
            display: 'flex', alignItems: 'flex-end', gap: 1,
          }}
        >
          <TextField
            fullWidth
            multiline
            maxRows={4}
            value={input}
            onChange={(e) => setInput(e.target.value)}
            placeholder="Ask anything about your project..."
            size="small"
            disabled={isTyping}
            onKeyDown={(e) => {
              if (e.key === 'Enter' && !e.shiftKey) { e.preventDefault(); handleSend(); }
            }}
            sx={{
              '& .MuiOutlinedInput-root': {
                borderRadius: 3,
                bgcolor: isLight ? alpha('#3d5afe', 0.04) : alpha('#3d5afe', 0.1),
              },
            }}
          />
          <Tooltip title="Send (Enter)">
            <span>
              <IconButton
                color="primary"
                type="submit"
                disabled={isTyping || !input.trim()}
                sx={{
                  mb: 0.25, width: 40, height: 40,
                  bgcolor: input.trim() && !isTyping ? 'primary.main' : 'transparent',
                  color: input.trim() && !isTyping ? 'white' : 'text.disabled',
                  borderRadius: 2,
                  '&:hover': { bgcolor: 'primary.dark', color: 'white' },
                  transition: 'all 0.2s',
                }}
              >
                <SendIcon sx={{ fontSize: 18 }} />
              </IconButton>
            </span>
          </Tooltip>
        </Box>

        {/* Branding */}
        <Box sx={{ px: 2.5, pb: 1.75, flexShrink: 0, textAlign: 'center' }}>
          <Typography variant="caption" color="text.disabled" sx={{ fontSize: '0.62rem' }}>
            Powered by Cerulea AI. May make mistakes. Always verify critical details.
          </Typography>
        </Box>
      </Drawer>
    </>
  );
}
