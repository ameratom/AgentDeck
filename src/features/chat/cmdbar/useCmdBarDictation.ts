import { useCallback, useEffect, useMemo, useRef, useState } from "react";

type SpeechRecognitionResultLike = {
  isFinal: boolean;
  0: {
    transcript: string;
  };
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
  onerror: (() => void) | null;
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

export function useCmdBarDictation(onFinalTranscript: (text: string) => void) {
  const speechSupported = useMemo(() => speechRecognitionCtor() !== null, []);
  const [listening, setListening] = useState(false);
  const recognitionRef = useRef<SpeechRecognitionLike | null>(null);

  const stopDictation = useCallback(() => {
    recognitionRef.current?.stop();
    recognitionRef.current = null;
    setListening(false);
  }, []);

  const toggleDictation = useCallback(() => {
    if (!speechSupported) {
      return;
    }

    if (listening) {
      stopDictation();
      return;
    }

    const Ctor = speechRecognitionCtor();
    if (!Ctor) {
      return;
    }

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
        onFinalTranscript(finalText.trim());
      }
    };

    recognition.onend = () => {
      recognitionRef.current = null;
      setListening(false);
    };

    recognition.onerror = () => {
      recognitionRef.current = null;
      setListening(false);
    };

    recognitionRef.current = recognition;
    recognition.start();
    setListening(true);
  }, [listening, onFinalTranscript, speechSupported, stopDictation]);

  useEffect(() => () => stopDictation(), [stopDictation]);

  return { speechSupported, listening, toggleDictation, stopDictation };
}
