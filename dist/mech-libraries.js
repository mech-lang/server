var mech_libraries = (function (exports) {
'use strict';

/*! *****************************************************************************
Copyright (c) Microsoft Corporation. All rights reserved.
Licensed under the Apache License, Version 2.0 (the "License"); you may not use
this file except in compliance with the License. You may obtain a copy of the
License at http://www.apache.org/licenses/LICENSE-2.0

THIS CODE IS PROVIDED ON AN *AS IS* BASIS, WITHOUT WARRANTIES OR CONDITIONS OF ANY
KIND, EITHER EXPRESS OR IMPLIED, INCLUDING WITHOUT LIMITATION ANY IMPLIED
WARRANTIES OR CONDITIONS OF TITLE, FITNESS FOR A PARTICULAR PURPOSE,
MERCHANTABLITY OR NON-INFRINGEMENT.

See the Apache Version 2.0 License for specific language governing permissions
and limitations under the License.
***************************************************************************** */
/* global Reflect, Promise */

var extendStatics = Object.setPrototypeOf ||
    ({ __proto__: [] } instanceof Array && function (d, b) { d.__proto__ = b; }) ||
    function (d, b) { for (var p in b) if (b.hasOwnProperty(p)) d[p] = b[p]; };

function __extends(d, b) {
    extendStatics(d, b);
    function __() { this.constructor = d; }
    d.prototype = b === null ? Object.create(b) : (__.prototype = b.prototype, new __());
}

////////////////////////////////////////////////////////////////////////////////
// Library
////////////////////////////////////////////////////////////////////////////////
var Library = /** @class */ (function () {
    function Library(_program) {
        this._program = _program;
    }
    Library.register = function (id, library) {
        if (this._registry[id]) {
            if (this._registry[id] === library)
                return;
            throw new Error("Attempting to overwrite existing library with id '" + id + "'");
        }
        this._registry[id] = library;
    };
    Library.unregister = function (id) {
        delete this._registry[id];
    };
    Library.get = function (id) {
        var library = this._registry[id];
        if (library)
            return library;
    };
    Library.attach = function (program, libraryId) {
        var LibraryCtor = Library.get(libraryId);
        if (!LibraryCtor)
            throw new Error("Unable to attach unknown library '" + libraryId + "'.");
        if (program.libraries[libraryId])
            return program.libraries[libraryId];
        var library = new LibraryCtor(program);
        program.libraries[libraryId] = library;
        library.setup();
        program.attached(libraryId, library);
        return library;
    };
    Object.defineProperty(Library.prototype, "order", {
        get: function () {
            if (this._order)
                return this._order;
            return this._order = Object.keys(this.handlers);
        },
        enumerable: true,
        configurable: true
    });
    Object.defineProperty(Library.prototype, "program", {
        get: function () { return this._program; },
        enumerable: true,
        configurable: true
    });
    Library.prototype.setup = function () { };
    Library._registry = {};
    return Library;
}());
////////////////////////////////////////////////////////////////////////////////
// Handlers
////////////////////////////////////////////////////////////////////////////////
// Just a convenience fn for type hinting.
function handleTuples(handler) {
    return handler;
}


////////////////////////////////////////////////////////////////////////////////
// Helpers
////////////////////////////////////////////////////////////////////////////////

var commonjsGlobal = typeof window !== 'undefined' ? window : typeof global !== 'undefined' ? global : typeof self !== 'undefined' ? self : {};

(function (global, undefined) {
    "use strict";

    if (global.setImmediate) {
        return;
    }

    var nextHandle = 1; // Spec says greater than zero
    var tasksByHandle = {};
    var currentlyRunningATask = false;
    var doc = global.document;
    var registerImmediate;

    function setImmediate(callback) {
      // Callback can either be a function or a string
      if (typeof callback !== "function") {
        callback = new Function("" + callback);
      }
      // Copy function arguments
      var args = new Array(arguments.length - 1);
      for (var i = 0; i < args.length; i++) {
          args[i] = arguments[i + 1];
      }
      // Store and register the task
      var task = { callback: callback, args: args };
      tasksByHandle[nextHandle] = task;
      registerImmediate(nextHandle);
      return nextHandle++;
    }

    function clearImmediate(handle) {
        delete tasksByHandle[handle];
    }

    function run(task) {
        var callback = task.callback;
        var args = task.args;
        switch (args.length) {
        case 0:
            callback();
            break;
        case 1:
            callback(args[0]);
            break;
        case 2:
            callback(args[0], args[1]);
            break;
        case 3:
            callback(args[0], args[1], args[2]);
            break;
        default:
            callback.apply(undefined, args);
            break;
        }
    }

    function runIfPresent(handle) {
        // From the spec: "Wait until any invocations of this algorithm started before this one have completed."
        // So if we're currently running a task, we'll need to delay this invocation.
        if (currentlyRunningATask) {
            // Delay by doing a setTimeout. setImmediate was tried instead, but in Firefox 7 it generated a
            // "too much recursion" error.
            setTimeout(runIfPresent, 0, handle);
        } else {
            var task = tasksByHandle[handle];
            if (task) {
                currentlyRunningATask = true;
                try {
                    run(task);
                } finally {
                    clearImmediate(handle);
                    currentlyRunningATask = false;
                }
            }
        }
    }

    function installNextTickImplementation() {
        registerImmediate = function(handle) {
            process.nextTick(function () { runIfPresent(handle); });
        };
    }

    function canUsePostMessage() {
        // The test against `importScripts` prevents this implementation from being installed inside a web worker,
        // where `global.postMessage` means something completely different and can't be used for this purpose.
        if (global.postMessage && !global.importScripts) {
            var postMessageIsAsynchronous = true;
            var oldOnMessage = global.onmessage;
            global.onmessage = function() {
                postMessageIsAsynchronous = false;
            };
            global.postMessage("", "*");
            global.onmessage = oldOnMessage;
            return postMessageIsAsynchronous;
        }
    }

    function installPostMessageImplementation() {
        // Installs an event handler on `global` for the `message` event: see
        // * https://developer.mozilla.org/en/DOM/window.postMessage
        // * http://www.whatwg.org/specs/web-apps/current-work/multipage/comms.html#crossDocumentMessages

        var messagePrefix = "setImmediate$" + Math.random() + "$";
        var onGlobalMessage = function(event) {
            if (event.source === global &&
                typeof event.data === "string" &&
                event.data.indexOf(messagePrefix) === 0) {
                runIfPresent(+event.data.slice(messagePrefix.length));
            }
        };

        if (global.addEventListener) {
            global.addEventListener("message", onGlobalMessage, false);
        } else {
            global.attachEvent("onmessage", onGlobalMessage);
        }

        registerImmediate = function(handle) {
            global.postMessage(messagePrefix + handle, "*");
        };
    }

    function installMessageChannelImplementation() {
        var channel = new MessageChannel();
        channel.port1.onmessage = function(event) {
            var handle = event.data;
            runIfPresent(handle);
        };

        registerImmediate = function(handle) {
            channel.port2.postMessage(handle);
        };
    }

    function installReadyStateChangeImplementation() {
        var html = doc.documentElement;
        registerImmediate = function(handle) {
            // Create a <script> element; its readystatechange event will be fired asynchronously once it is inserted
            // into the document. Do so, thus queuing up the task. Remember to clean up once it's been called.
            var script = doc.createElement("script");
            script.onreadystatechange = function () {
                runIfPresent(handle);
                script.onreadystatechange = null;
                html.removeChild(script);
                script = null;
            };
            html.appendChild(script);
        };
    }

    function installSetTimeoutImplementation() {
        registerImmediate = function(handle) {
            setTimeout(runIfPresent, 0, handle);
        };
    }

    // If supported, we should attach to the prototype of global, since that is where setTimeout et al. live.
    var attachTo = Object.getPrototypeOf && Object.getPrototypeOf(global);
    attachTo = attachTo && attachTo.setTimeout ? attachTo : global;

    // Don't get fooled by e.g. browserify environments.
    if ({}.toString.call(global.process) === "[object process]") {
        // For Node.js before 0.9
        installNextTickImplementation();

    } else if (canUsePostMessage()) {
        // For non-IE10 modern browsers
        installPostMessageImplementation();

    } else if (global.MessageChannel) {
        // For web workers, where supported
        installMessageChannelImplementation();

    } else if (doc && "onreadystatechange" in doc.createElement("script")) {
        // For IE 6–8
        installReadyStateChangeImplementation();

    } else {
        // For older browsers
        installSetTimeoutImplementation();
    }

    attachTo.setImmediate = setImmediate;
    attachTo.clearImmediate = clearImmediate;
}(typeof self === "undefined" ? typeof commonjsGlobal === "undefined" ? commonjsGlobal : commonjsGlobal : self));

var EMPTY = [];
var HTML = /** @class */ (function (_super) {
    __extends(HTML, _super);
    function HTML() {
        var _this = _super !== null && _super.apply(this, arguments) || this;
        /** Instances are the physical DOM elements representing table elements. */
        _this._instances = [];
        _this._paths = [];
        _this._isChanging = false;
        _this.changed = function () {
            _this.rerender();
            _this._isChanging = false;
        };
        _this.handlers = {
            "export instances": handleTuples(function (_a) {
                var adds = _a.adds, removes = _a.removes;
                for (var _i = 0, _b = removes || EMPTY; _i < _b.length; _i++) {
                    var remove = _b[_i];
                    //this.removeInstance(instanceId);
                }
                for (var _c = 0, _d = adds || EMPTY; _c < _d.length; _c++) {
                    var _e = _d[_c], table = _e[0], row = _e[1], column = _e[2], value = _e[3];
                    if (table == 1819042146) {
                        if (column == 120) {
                            column = 1;
                        }
                        if (column == 121) {
                            column = 2;
                        }
                        _this.addInstance(row, column, value);
                    }
                }
            })
        };
        _this._keyMap = {
            9: "tab",
            13: "enter",
            16: "shift",
            17: "control",
            18: "alt",
            27: "escape",
            32: "space",
            37: "left",
            38: "up",
            39: "right",
            40: "down",
            91: "meta"
        };
        return _this;
    }
    HTML.prototype.setup = function () {
        // If we're not in a browser environment, this library does nothing
        if (typeof document === "undefined") {
            this.handlers = {};
            return;
        }
        this._container = document.createElement("div");
        this._container.setAttribute("program", this.program.name);
        document.body.appendChild(this._container);
        var editor = document.createElement("div");
        editor.setAttribute("class", "editor");
        this._container.appendChild(editor);
        var textarea = document.createElement("textarea");
        textarea.setAttribute("id", "editor-text-area");
        editor.appendChild(textarea);
        var canvas = this._canvas = document.createElement("canvas");
        canvas.setAttribute("width", "500");
        canvas.setAttribute("height", "500");
        canvas.style.backgroundColor = 'rgb(226, 79, 94)';
        this._container.appendChild(canvas);
        window.addEventListener("click", this._mouseEventHandler("click"));
        //window.addEventListener("change", this._changeEventHandler("change"));
        //window.addEventListener("input", this._inputEventHandler("change"));
        window.addEventListener("keyup", this._keyEventHandler("key-up"));
        //window.addEventListener("keyup", this._keyEventHandler("key-up"));
        var context = canvas.getContext('2d');
        if (context !== null) {
            var centerX = canvas.width / 2;
            var centerY = canvas.height / 2;
            var radius = 5;
            context.beginPath();
            context.arc(centerX, centerY, radius, 0, 2 * Math.PI, false);
            context.fillStyle = 'black';
            context.fill();
            context.lineWidth = 0;
            context.strokeStyle = '#000000';
            context.stroke();
        }
    };
    HTML.prototype.decorate = function (elem, value) {
        var e = elem;
        e.__element = value;
        e.__source = this;
        e.textContent = "" + value;
        this._container.appendChild(e);
        return e;
    };
    HTML.prototype.addInstance = function (row, column, value) {
        row = row - 1;
        column = column - 1;
        //if(id === null || id === "null") throw new Error(`Cannot create instance with null id for element '${elemId}'.`);
        if (this._instances[row] === undefined) {
            this._instances[row] = [];
        }
        /*
        let instance = this._instances[row][column];
        if (instance == undefined) {
          this._instances[row][column] = this.decorate(document.createElement("div"), value);
        } else {
          instance.textContent = `${value}`;
        }*/
        if (this._paths[row] === undefined) {
            this._paths[row] = [];
        }
        this._paths[row][column] = value;
        //let n = new Node();
        //this._container.appendChild(n);
        //if(instance) throw new Error(`Recreating existing instance '${id}'`);
        //if(ns) instance = this.decorate(document.createElementNS(""+ns, ""+tagname), elemId);
        //else instance = this.decorate(document.createElement(""+tagname), elemId);
        //if(!this._elementToInstances[elemId]) this._elementToInstances[elemId] = [id];
        //else this._elementToInstances[elemId].push(id);
        //return this._instances[id] = instance;
        this.changing();
    };
    HTML.prototype.rerender = function () {
        var canvas = this._canvas;
        var context = canvas.getContext("2d");
        context.clearRect(0, 0, canvas.width, canvas.height);
        var radius = 5;
        for (var _i = 0, _a = this._paths; _i < _a.length; _i++) {
            var path = _a[_i];
            var centerX = path[0] / 10;
            var centerY = path[1] / 10;
            context.beginPath();
            context.arc(centerX, centerY, radius, 0, 2 * Math.PI, false);
            context.fillStyle = 'black';
            context.fill();
            context.lineWidth = 1;
            context.strokeStyle = '#000000';
            context.stroke();
        }
    };
    HTML.prototype.changing = function () {
        if (!this._isChanging) {
            this._isChanging = true;
            setImmediate(this.changed);
        }
    };
    HTML.prototype._sendEvent = function (change) {
        console.log(change);
        this.program.send_transaction(change);
    };
    // ----------------------
    // BROWSER EVENT HANDLERS
    // ----------------------
    HTML.prototype._mouseEventHandler = function (tagname) {
        var _this = this;
        var table_id = 0x1a076b771;
        return function (event) {
            _this._sendEvent([[table_id, 1, 120, event.x],
                [table_id, 1, 121, event.y]]);
        };
    };
    HTML.prototype._keyEventHandler = function (tagname) {
        var _this = this;
        return function (event) {
            if (event.repeat)
                return;
            var target = event.target;
            var code = event.keyCode;
            var key = _this._keyMap[code];
            var value = target.value;
            if (value != undefined) {
                _this._sendEvent([[1, 1, 1, value]]);
            }
        };
    };
    HTML.id = "html";
    return HTML;
}(Library));

Library.register(HTML.id, HTML);
window["lib"] = Library;

var Connection = /** @class */ (function () {
    function Connection(ws) {
        var _this = this;
        this.ws = ws;
        this._queue = [];
        this.connected = false;
        this.handlers = {};
        this._closed = function (code, reason) {
            _this.connected = false;
            console.warn("Connection closed.", code, reason);
        };
        this._messaged = function (payload) {
            var parsed;
            try {
                parsed = JSON.parse(payload);
            }
            catch (err) {
                console.error("Received malformed WS message: '" + payload + "'.");
                return;
            }
            if (_this.handlers[parsed.type]) {
                //console.group(`Received ${parsed.type} from ${parsed.client}`);
                _this.handlers[parsed.type](parsed);
                //console.groupEnd();
            }
            else {
                console.warn("Received unhandled message of type: '" + parsed.type + "'.", parsed);
            }
        };
        ws.addEventListener("open", function () { return _this._opened(); });
        ws.addEventListener("close", function (event) { return _this._closed(event.code, event.reason); });
        ws.addEventListener("message", function (event) { return _this._messaged(event.data); });
    }
    Connection.prototype.send = function (type, data, client) {
        var _a;
        console.groupCollapsed("Sent");
        console.log(type, data, client);
        console.groupEnd();
        // This... feels weird. Do we actually expect to pack multiple message types in very frequently?
        data.client = client;
        var payload = JSON.stringify((_a = {}, _a[type] = data, _a));
        this._queue.push(payload);
        this._trySend();
    };
    Connection.prototype._trySend = function () {
        if (this.connected) {
            // @NOTE: this doesn't gracefully handle partial processing of the queue.
            while (this._queue.length) {
                var payload = this._queue.shift();
                if (payload === undefined) {
                    payload = "";
                }
                this.ws.send(payload);
            }
        }
    };
    Connection.prototype._opened = function () {
        console.log("Connection opened.");
        this.connected = true;
        //let diff = {adds: [[1, 3, 1, 10], [1, 3, 2, 15]], removes: []};
        //this.send("Transaction", diff);
        this._trySend();
    };
    return Connection;
}());

var RemoteProgram = /** @class */ (function () {
    function RemoteProgram(name, send) {
        if (name === void 0) { name = "Remote Client"; }
        this.name = name;
        this.send = send;
        this.libraries = {};
        this.handlers = {};
    }
    RemoteProgram.prototype.attach = function (libraryId) {
        return Library.attach(this, libraryId);
    };
    RemoteProgram.prototype.attached = function (libraryId, library) {
        for (var handlerName in library.handlers) {
            this.handlers[libraryId + "/" + handlerName] = library.handlers[handlerName];
        }
    };
    RemoteProgram.prototype.send_transaction = function (transaction) {
        this.send("Transaction", { adds: transaction, removes: [] });
        return this;
    };
    RemoteProgram.prototype.handleDiff = function (diff) {
        for (var type in this.handlers) {
            this.handlers[type](diff);
        }
    };
    return RemoteProgram;
}());
var MultiplexedConnection = /** @class */ (function (_super) {
    __extends(MultiplexedConnection, _super);
    function MultiplexedConnection() {
        var _this = _super !== null && _super.apply(this, arguments) || this;
        _this.programs = {};
        _this.panes = {};
        _this.handlers = {
            "init": function (_a) {
                var client = _a.client;
                if (_this.programs[client])
                    throw new Error("Unable to initialize existing program: '" + client + "'.");
                var program = _this.programs[client] = new RemoteProgram(client, function (type, diff) { return _this.send(type, diff, client); });
                var html = program.attach("html");
            },
            "diff": function (diff) {
                var program = _this.programs[diff.client];
                if (!program)
                    throw new Error("Unable to handle diff for unitialized program: '" + diff.client + "'.");
                program.handleDiff(diff);
            }
        };
        return _this;
    }
    MultiplexedConnection.prototype.addPane = function (name, container) {
        if (this.panes[name] && this.panes[name] !== container) {
            console.warn("Overwriting container for existing pane '" + name + "'");
        }
        this.panes[name] = container;
        container.classList.add("program-pane");
    };
    return MultiplexedConnection;
}(Connection));
var host = location.hostname == "" ? "localhost" : location.hostname;
var connection = new MultiplexedConnection(new WebSocket("ws://" + host + ":3012"));
console.log(connection);

return exports;

}({}));
