"use strict";
Object.defineProperty(exports, "__esModule", { value: true });
exports.env = void 0;
let env = require("../env.json");
exports.env = env;
if (process.argv[2]) {
    exports.env = env = env[process.argv[2]];
}
else {
    exports.env = env = env["dev"];
}
;
