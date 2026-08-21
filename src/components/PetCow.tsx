import {
  useEffect,
  useRef,
  useState,
  type MouseEvent as ReactMouseEvent,
  type PointerEvent as ReactPointerEvent,
} from "react";
import pinkyCow from "../assets/pinky-cow.webp";
import { PINKY_MOO_DURATION_MS, playCowMoo } from "../lib/petAudio";

interface Position {
  x: number;
  y: number;
}

type Reaction = "idle" | "speaking" | "walking" | "jumping";

const PET_SIZE = 142;
const POSITION_KEY = "moco-pinky-position";

function clampPosition(position: Position): Position {
  return {
    x: Math.min(Math.max(10, position.x), Math.max(10, window.innerWidth - PET_SIZE - 10)),
    y: Math.min(Math.max(54, position.y), Math.max(54, window.innerHeight - PET_SIZE - 16)),
  };
}

function initialPosition(): Position {
  try {
    const saved = JSON.parse(localStorage.getItem(POSITION_KEY) ?? "null") as Position | null;
    if (saved && Number.isFinite(saved.x) && Number.isFinite(saved.y))
      return clampPosition(saved);
  } catch {
    // A malformed preference should never prevent Pinky from appearing.
  }
  return clampPosition({
    x: window.innerWidth - PET_SIZE - 34,
    y: window.innerHeight - PET_SIZE - 74,
  });
}

export function PetCow() {
  const [position, setPosition] = useState(initialPosition);
  const [dragging, setDragging] = useState(false);
  const [reaction, setReactionState] = useState<Reaction>("speaking");
  const [reactionCount, setReactionCount] = useState(0);
  const [bubbleText, setBubbleText] = useState("Moo! I’m Moco.");
  const [showBubble, setShowBubble] = useState(true);
  const positionRef = useRef(position);
  const draggingRef = useRef(false);
  const reactionRef = useRef<Reaction>("speaking");
  const reactionTimers = useRef<number[]>([]);
  const ignoreClick = useRef(false);
  const drag = useRef<{
    pointerId: number;
    startX: number;
    startY: number;
    origin: Position;
    moved: boolean;
  } | undefined>(undefined);

  const setReaction = (next: Reaction) => {
    reactionRef.current = next;
    setReactionState(next);
  };

  const clearReactionTimers = () => {
    reactionTimers.current.forEach(window.clearTimeout);
    reactionTimers.current = [];
  };

  const finishWithJump = () => {
    clearReactionTimers();
    reactionTimers.current.push(
      window.setTimeout(() => {
        setShowBubble(false);
        setReaction("jumping");
        reactionTimers.current.push(
          window.setTimeout(() => setReaction("idle"), 760),
        );
      }, PINKY_MOO_DURATION_MS),
    );
  };

  const reactToPet = () => {
    setReactionCount((value) => value + 1);
    setBubbleText("Moooo! 💗");
    setShowBubble(true);
    setReaction("speaking");
    finishWithJump();
    playCowMoo();
  };

  useEffect(() => {
    finishWithJump();
    const resize = () => {
      const next = clampPosition(positionRef.current);
      positionRef.current = next;
      setPosition(next);
    };
    window.addEventListener("resize", resize);
    return () => {
      clearReactionTimers();
      window.removeEventListener("resize", resize);
    };
  }, []);

  useEffect(() => {
    let cancelled = false;
    let wanderTimer = 0;
    let finishTimer = 0;

    const scheduleWander = () => {
      wanderTimer = window.setTimeout(() => {
        if (cancelled) return;
        if (!draggingRef.current && reactionRef.current === "idle") {
          const current = positionRef.current;
          const distance = 30 + Math.round(Math.random() * 34);
          const direction = Math.random() > 0.5 ? 1 : -1;
          let next = clampPosition({ x: current.x + distance * direction, y: current.y });
          if (Math.abs(next.x - current.x) < 12)
            next = clampPosition({ x: current.x - distance * direction, y: current.y });

          setReaction("walking");
          positionRef.current = next;
          setPosition(next);
          localStorage.setItem(POSITION_KEY, JSON.stringify(next));
          finishTimer = window.setTimeout(() => {
            if (reactionRef.current === "walking") setReaction("idle");
            scheduleWander();
          }, 1_050);
        } else {
          scheduleWander();
        }
      }, 5_500 + Math.round(Math.random() * 3_500));
    };

    scheduleWander();
    return () => {
      cancelled = true;
      window.clearTimeout(wanderTimer);
      window.clearTimeout(finishTimer);
    };
  }, []);

  const pointerDown = (event: ReactPointerEvent<HTMLButtonElement>) => {
    if (event.button !== 0) return;
    event.currentTarget.setPointerCapture(event.pointerId);
    drag.current = {
      pointerId: event.pointerId,
      startX: event.clientX,
      startY: event.clientY,
      origin: positionRef.current,
      moved: false,
    };
    draggingRef.current = true;
    setDragging(true);
    setReaction("walking");
  };

  const pointerMove = (event: ReactPointerEvent<HTMLButtonElement>) => {
    const current = drag.current;
    if (!current || current.pointerId !== event.pointerId) return;
    const x = event.clientX - current.startX;
    const y = event.clientY - current.startY;
    if (Math.abs(x) + Math.abs(y) > 5) current.moved = true;
    const next = clampPosition({ x: current.origin.x + x, y: current.origin.y + y });
    positionRef.current = next;
    setPosition(next);
  };

  const pointerUp = (event: ReactPointerEvent<HTMLButtonElement>) => {
    const current = drag.current;
    if (!current || current.pointerId !== event.pointerId) return;
    drag.current = undefined;
    draggingRef.current = false;
    setDragging(false);
    ignoreClick.current = current.moved;
    localStorage.setItem(POSITION_KEY, JSON.stringify(positionRef.current));
    if (current.moved) {
      setReaction("jumping");
      clearReactionTimers();
      reactionTimers.current.push(
        window.setTimeout(() => setReaction("idle"), 760),
      );
    }
  };

  const pointerCancel = () => {
    drag.current = undefined;
    draggingRef.current = false;
    ignoreClick.current = true;
    setDragging(false);
    setReaction("idle");
  };

  const clickPet = (event: ReactMouseEvent<HTMLButtonElement>) => {
    if (ignoreClick.current) {
      ignoreClick.current = false;
      event.preventDefault();
      return;
    }
    reactToPet();
  };

  return (
    <aside
      className={`pet-cow is-${reaction} ${dragging ? "is-dragging" : ""}`}
      style={{ transform: `translate3d(${position.x}px, ${position.y}px, 0)` }}
      aria-label="Pinky, Moco's pet cow"
    >
      {showBubble && (
        <div className="pet-cow-bubble" role="status">
          {bubbleText}
        </div>
      )}
      <button
        className="pet-cow-drag"
        type="button"
        aria-label="Tap Pinky to moo, or drag her around"
        title="Tap me to moo, or drag me anywhere"
        onClick={clickPet}
        onPointerDown={pointerDown}
        onPointerMove={pointerMove}
        onPointerUp={pointerUp}
        onPointerCancel={pointerCancel}
      >
        <span className="pet-cow-shadow" aria-hidden="true" />
        <img
          key={`cow-${reactionCount}`}
          className="pet-cow-sprite"
          src={pinkyCow}
          alt=""
          draggable={false}
        />
        <span className="pet-cow-speech-dots" aria-hidden="true">
          <span />
          <span />
          <span />
        </span>
        <span className="pet-cow-sparkles" aria-hidden="true">
          <span>✦</span>
          <span>✦</span>
          <span>♥</span>
        </span>
        <span className="pet-cow-heart" key={`heart-${reactionCount}`} aria-hidden="true">
          ♥
        </span>
      </button>
    </aside>
  );
}
