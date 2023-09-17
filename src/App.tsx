import { useEffect, useRef, useState } from "react";
import { invoke } from "@tauri-apps/api/tauri";
import { listen } from "@tauri-apps/api/event";
import "./App.css";

import { register } from '@tauri-apps/api/globalShortcut'

register('Alt+Enter', () => {
  invoke("show");
})

type Hint = {
  text: string;
  x: number;
  y: number;
  x_offset:number;
  y_offset:number;
  width:number;
  height:number;
  hint: string;
  control: string;
  parent: string;
}

function App() {
  const [results, setResults] = useState([] as Hint[]);
  const [input, setInput] = useState("");
  const [selectedIndex, setSelectedIndex] = useState(0);
  const [spaceDown, setSpaceDown] = useState(false);
  const [pressedNav, setPressedNav] = useState(false);
  const [finding, setFinding] = useState(true);

  const inputArea = useRef<HTMLDivElement>(null);
  const inputBox = useRef<HTMLInputElement>(null);

  async function invoke_hide_and_clear() {
    console.log("invoking hide");
    await invoke("hide");
    setResults([]);
    setInput("");
  }

  useEffect(() => {
    function handleClickOutside(event: MouseEvent) {
      if (inputArea.current && !inputArea.current.contains(event.target as Node)) {
        invoke_hide_and_clear();
      }
    }

    document.addEventListener('click', handleClickOutside);
    return () => {
      document.removeEventListener('click', handleClickOutside);
    };
  }, [inputArea]);

  listen("update_results", (event) => {
    setResults(event.payload as Hint[]);
    setSelectedIndex(old => Math.min(old, results.length - 1));
    setSelectedIndex(old => Math.max(old, 0));
    setFinding(false);

  });

  listen("show", (_) => {
    setFinding(true);
  });

  async function update_input(newValue: string) {
    setInput(_ => newValue);
    setFinding(true);
    await invoke("update_input", { input: newValue });
  }

  async function invoke_choice(action: string) {

    var hint = results[selectedIndex].hint;

    await invoke_hide_and_clear();

    switch (action) {
      case "LeftClick":
        await invoke("choice", { choice: hint, action: "LeftClick" });
        break;
      case "RightClick":
        await invoke("choice", { choice: hint, action: "RightClick" });
        break;

    }
  }


  async function input_keydown(e: React.KeyboardEvent<HTMLInputElement>) {
    console.log("down:" + e.key);
    if (e.key === " ") {
      e.preventDefault();
      setSpaceDown(true);
    } else if ((e.key.toUpperCase() === "J" && spaceDown) || e.key == "ArrowDown") {
      e.preventDefault();
      setSelectedIndex(old => Math.min(old + 1, results.length - 1));
      setPressedNav(true);
      const selectedDiv = document.querySelector('.result-selected');
      if (selectedDiv) {
        selectedDiv.scrollIntoView({ behavior: 'smooth', block: 'center' });
      }

    } else if ((e.key.toUpperCase() === "K" && spaceDown) || e.key == "ArrowUp") {
      e.preventDefault();
      setSelectedIndex(old => Math.max(old - 1, 0));
      setPressedNav(true);
      const selectedDiv = document.querySelector('.result-selected');
      if (selectedDiv) {
        selectedDiv.scrollIntoView({ behavior: 'smooth', block: 'center' });
      }
    } else if (e.key === "Escape") {
      await invoke_hide_and_clear();
    } else if (e.key === "Enter" && e.ctrlKey) {
      invoke_choice("RightClick");
    } else if (e.key === "Enter") {
      invoke_choice("LeftClick");
    }

  }
  async function input_keyup(e: React.KeyboardEvent<HTMLInputElement>) {
    if (e.key === " ") {
      if (pressedNav) {
        e.preventDefault();
      } else {
        setInput(old => old + " ");
      }
      setSpaceDown(false);
      setPressedNav(false);
    }
  }



  return (
    <div className="container">
      <div className="input-area" ref={inputArea}>
        <input
          autoFocus={true}
          spellCheck={false}
          autoComplete="off"
          autoCorrect="off"
          autoCapitalize="off"
          onChange={(e) => update_input(e.currentTarget.value)}
          value={input}
          placeholder="Search for element names or hint shortcut"
          onKeyDown={(e) => input_keydown(e)}
          onKeyUp={(e) => input_keyup(e)}
          onBlur={() => inputBox.current?.focus()}
          ref={inputBox}
        />
        <label className="input-label">Press <a className="highlight">Enter</a> to left click, <a className="highlight">Ctrl+Enter</a> to right click. Hold <a className="highlight">Space+J/K</a> or <a className="highlight">Down/Up</a> to scroll.<span style={{ marginLeft: '10px' }}>{finding ? <div className="loader"></div> : "Found " + results.length}</span></label>

        {results.length > 0 &&
          <div className="holder">
            {results.map((result, i) => {

              return (
                <div className={i === selectedIndex ? "result result-selected" : "result"}><div className="result-left">{result.text} ({result.hint})</div><div className="result-right">{result.parent == "taskbar" ? "taskbar | " : ""}{result.control}{result.x},{result.y}</div></div>
              );
            })}
          </div>}
      </div>
      {results.map((result, i) => {
        let left=result.x+result.x_offset ;
        let top=result.y+result.y_offset ;
        let wid = result.width;
        let hei = result.height;
        const style = { left: left+ "px", 
        top: top+ "px",
        width:wid==0?'auto':wid+'px', 
        height:hei==0?'auto':hei+'px'
      };
        return (
          <div className={i==selectedIndex?"hint-holder hint-holder-selected":"hint-holder"} style={style}>
          <div className={i === selectedIndex ? "hint hint-selected" : "hint"}>{result.hint}</div>
          </div>
        );
      })}
    </div>
  );
}

export default App;
