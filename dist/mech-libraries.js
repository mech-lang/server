var mech_libraries = (function (exports) {
'use strict';

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
            console.log(payload);
            var deserialized = JSON.parse(payload);
            console.log(deserialized);
        };
        ws.addEventListener("open", function () { return _this._opened(); });
        ws.addEventListener("close", function (event) { return _this._closed(event.code, event.reason); });
        ws.addEventListener("message", function (event) { return _this._messaged(event.data); });
    }
    Connection.prototype.send = function (type, data, client) {
        console.groupCollapsed("Sent");
        console.log(type, data, client);
        console.groupEnd();
        // This... feels weird. Do we actually expect to pack multiple message types in very frequently?
        data.client = client;
        var payload = JSON.stringify((_a = {}, _a[type] = data, _a));
        this._queue.push(payload);
        this._trySend();
        var _a;
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
        console.log("Opened");
        this.connected = true;
        var diff = { adds: [[1, 3, 1, 10], [1, 3, 2, 15]], removes: [] };
        this.send("Transaction", diff);
        this._trySend();
    };
    return Connection;
}());

var connection = new Connection(new WebSocket("ws://localhost:3012"));
console.log(connection);

return exports;

}({}));
