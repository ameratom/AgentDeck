import { useCallback, useEffect, useMemo, useRef, useState } from "react";

type SpeechRecognitionResultLike = {
  isFinal: boolean;
  0: {
    transcript: string;
  };
};

type SpeechRecognitionErrorEventLike = {
  error?: string;
  message?: string;
};

type SpeechRecognitionEventLike = {
  resultIndex: number;
  results: {
    length: number;
    [index: number]: SpeechRecognitionResultLike;
  };
};

type SpeechRecognitionLike = {
  continuous: boolean;
  interimResults: boolean;
  lang: string;
  onresult: ((event: SpeechRecognitionEventLike) => void) | null;
  onend: (() => void) | null;
  onerror: ((event: SpeechRecognitionErrorEventLike) => void) | null;
  start: () => void;
  stop: () => void;
};

type SpeechRecognitionConstructor = new () => SpeechRecognitionLike;

type SpeechWindow = Window & {
  SpeechRecognition?: SpeechRecognitionConstructor;
  webkitSpeechRecognition?: SpeechRecognitionConstructor;
};

function speechRecognitionCtor(): SpeechRecognitionConstructor | null {
  const win = window as SpeechWindow;
  return win.SpeechRecognition ?? win.webkitSpeechRecognition ?? null;
}

export function formatDictationError(error: string | undefined): string {
  switch (error) {
    case "not-allowed":
      return "Microphone access denied. Allow speech recognition in System Settings.";
    case "service-not-allowed":
      return "Speech recognition is disabled for this browser context.";
    case "no-speech":
      return "No speech detected. Try again closer to the microphone.";
    case "audio-capture":
      return "No microphone available for dictation.";
    case "network":
      return "Dictation failed due to a network error.";
    case "aborted":
      return "Dictation stopped.";
    default:
      return error
        ? `Dictation failed (${error}).`
        : "Dictation failed. Try again.";
  }
}

export function useCmdBarDictation(onFinalTranscript: (text: string) => void) {
  const speechSupported = useMemo(() => speechRecognitionCtor() !== null, []);
  const [listening, setListening] = useState(false);
  const [dictationError, setDictationError] = useState<string | null>(null);
  const recognitionRef = useRef<SpeechRecognitionLike | null>(null);

  const stopDictation = useCallback(() => {
    recognitionRef.current?.stop();
    recognitionRef.current = null;
    setListening(false);
  }, []);

  const clearDictationError = useCallback(() => {
    setDictationError(null);
  }, []);

  const toggleDictation = useCallback(() => {
    if (!speechSupported) {
      setDictationError("Dictation is not supported in this browser.");
      return;
    }

    if (listening) {
      stopDictation();
      return;
    }

    const Ctor = speechRecognitionCtor();
    if (!Ctor) {
      setDictationError("Dictation is not supported in this browser.");
      return;
    }

    setDictationError(null);

    const recognition = new Ctor();
    recognition.continuous = true;
    recognition.interimResults = true;
    recognition.lang = navigator.language || "en-US";

    recognition.onresult = (event: SpeechRecognitionEventLike) => {
      let finalText = "";
      for (let index = event.resultIndex; index < event.results.length; index++) {
        if (event.results[index].isFinal) {
          finalText += event.results[index][0].transcript;
        }
      }
      if (finalText.trim()) {
        setDictationError(null);
        onFinalTranscript(finalText.trim());
      }
    };

    recognition.onend = () => {
      recognitionRef.current = null;
      setListening(false);
    };

    recognition.onerror = (event: SpeechRecognitionErrorEventLike) => {
      recognitionRef.current = null;
      setListening(false);
      if (event.error !== "aborted") {
        setDictationError(formatDictationError(event.error));
      }
    };

    try {
      recognitionRef.current = recognition;
      recognition.start();
      setListening(true);
    } catch {
      recognitionRef.current = null;
      setListening(false);
      setDictationError("Unable to start dictation.");
    }
  }, [listening, onFinalTranscript, speechSupported, stopDictation]);

  useEffect(() => () => stopDictation(), [stopDictation]);

  return {
    speechSupported,
    listening,
    dictationError,
    clearDictationError,
    toggleDictation,
    stopDictation,
  };
}