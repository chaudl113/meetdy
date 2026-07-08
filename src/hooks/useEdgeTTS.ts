import { useCallback, useRef, useState } from "react";
import { toast } from "sonner";
import { commands } from "@/bindings";

// Popular Vietnamese voices
export const EDGE_TTS_VOICES = [
  { value: "vi-VN-HoaiMyNeural", label: "Hoài My (VI, Female)" },
  { value: "vi-VN-NamMinhNeural", label: "Nam Minh (VI, Male)" },
  { value: "en-US-JennyNeural", label: "Jenny (EN, Female)" },
  { value: "en-US-GuyNeural", label: "Guy (EN, Male)" },
  { value: "ja-JP-NanamiNeural", label: "Nanami (JA, Female)" },
  { value: "zh-CN-XiaoxiaoNeural", label: "Xiaoxiao (ZH, Female)" },
  { value: "ko-KR-SunHiNeural", label: "Sun-Hi (KO, Female)" },
] as const;

export function useEdgeTTS() {
  const [isPlaying, setIsPlaying] = useState(false);
  const [isLoading, setIsLoading] = useState(false);
  const audioRef = useRef<HTMLAudioElement | null>(null);

  const speak = useCallback(async (text: string, voice = "vi-VN-HoaiMyNeural", rate = 0) => {
    if (!text.trim()) return;

    // Stop any playing audio
    if (audioRef.current) {
      audioRef.current.pause();
      audioRef.current = null;
    }

    const FALLBACK_VOICE = "en-US-JennyNeural";

    const tryPlay = async (v: string): Promise<boolean> => {
      const result = await commands.edgeTtsSpeak(text, v, rate);
      if (result.status === "ok") {
        const mp3Base64 = result.data;
        const audio = new Audio(`data:audio/mp3;base64,${mp3Base64}`);
        audioRef.current = audio;
        audio.onended = () => {
          setIsPlaying(false);
          audioRef.current = null;
        };
        audio.onerror = () => {
          setIsPlaying(false);
          audioRef.current = null;
        };
        setIsPlaying(true);
        await audio.play();
        return true;
      }
      return false;
    };

    setIsLoading(true);
    try {
      const ok = await tryPlay(voice);
      if (!ok && voice !== FALLBACK_VOICE) {
        toast.warning(`TTS voice "${voice}" failed, trying English fallback.`);
        const fallbackOk = await tryPlay(FALLBACK_VOICE);
        if (!fallbackOk) {
          toast.error("TTS failed: could not play audio.");
        }
      } else if (!ok) {
        toast.error("TTS failed: could not play audio.");
      }
    } catch (err) {
      toast.error(`TTS error: ${err}`);
    } finally {
      setIsLoading(false);
    }
  }, []);

  const stop = useCallback(() => {
    if (audioRef.current) {
      audioRef.current.pause();
      audioRef.current = null;
    }
    setIsPlaying(false);
  }, []);

  return { speak, stop, isPlaying, isLoading };
}
