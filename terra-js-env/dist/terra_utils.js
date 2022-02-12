"use strict";
var __createBinding = (this && this.__createBinding) || (Object.create ? (function(o, m, k, k2) {
    if (k2 === undefined) k2 = k;
    Object.defineProperty(o, k2, { enumerable: true, get: function() { return m[k]; } });
}) : (function(o, m, k, k2) {
    if (k2 === undefined) k2 = k;
    o[k2] = m[k];
}));
var __setModuleDefault = (this && this.__setModuleDefault) || (Object.create ? (function(o, v) {
    Object.defineProperty(o, "default", { enumerable: true, value: v });
}) : function(o, v) {
    o["default"] = v;
});
var __importStar = (this && this.__importStar) || function (mod) {
    if (mod && mod.__esModule) return mod;
    var result = {};
    if (mod != null) for (var k in mod) if (k !== "default" && Object.prototype.hasOwnProperty.call(mod, k)) __createBinding(result, mod, k);
    __setModuleDefault(result, mod);
    return result;
};
var __awaiter = (this && this.__awaiter) || function (thisArg, _arguments, P, generator) {
    function adopt(value) { return value instanceof P ? value : new P(function (resolve) { resolve(value); }); }
    return new (P || (P = Promise))(function (resolve, reject) {
        function fulfilled(value) { try { step(generator.next(value)); } catch (e) { reject(e); } }
        function rejected(value) { try { step(generator["throw"](value)); } catch (e) { reject(e); } }
        function step(result) { result.done ? resolve(result.value) : adopt(result.value).then(fulfilled, rejected); }
        step((generator = generator.apply(thisArg, _arguments || [])).next());
    });
};
Object.defineProperty(exports, "__esModule", { value: true });
exports.Address = void 0;
const terra_js_1 = require("@terra-money/terra.js");
const fs = __importStar(require("fs"));
const env_helper_1 = require("./env_helper");
// Wrapper for Query and Transaction objects (used to build a common Proxy on top of them)
class LCDClientWrapper {
    constructor(terra, wallet, contractAddress) {
        this.terra = terra;
        this.wallet = wallet;
        this.contractAddress = contractAddress;
    }
    execute(msgName, msgArgs) {
        console.log("execute not implemented");
    }
}
/// Execute Msg Handler
/// Removes a lot of code overhead
class Transaction extends LCDClientWrapper {
    signAndBroadcast(msgs, memo) {
        return __awaiter(this, void 0, void 0, function* () {
            const tx = yield this.wallet.createAndSignTx({
                msgs: msgs,
                memo: memo
            });
            return yield this.terra.tx.broadcast(tx);
        });
    }
    execute(msgName, msgArgs) {
        return __awaiter(this, void 0, void 0, function* () {
            let msg = { [msgName]: Object.assign({}, msgArgs)
            };
            const execute = new terra_js_1.MsgExecuteContract(this.wallet.key.accAddress, // sender
            this.contractAddress, Object.assign({}, msg));
            let response = yield this.signAndBroadcast([execute], "")
                .catch((response) => {
                if ((0, terra_js_1.isTxError)(response)) {
                    throw new Error(`store code failed. code: ${response.code}, codespace: ${response.codespace}, raw_log: ${response.raw_log}`);
                }
                else {
                    console.log(response["response"]["data"]);
                }
            });
            return response;
        });
    }
}
/// Query Msg Handler
/// Removes a lot of code overhead
class Query extends LCDClientWrapper {
    execute(msgName, msgArgs) {
        return __awaiter(this, void 0, void 0, function* () {
            let msg = { [msgName]: Object.assign({}, msgArgs)
            };
            let response = yield this.terra.wasm.contractQuery(this.contractAddress, msg);
            return response;
        });
    }
}
/// Allows one to query and execute contracts without too much overhead
class Contract {
    constructor(handler, contractAddress) {
        this.execute = createWrapperProxy(new Transaction(handler.terra, handler.wallet, contractAddress));
        this.query = createWrapperProxy(new Query(handler.terra, handler.wallet, contractAddress));
    }
}
/// Wrapper around a (LCDClient, Wallet) pair. 
/// Stores every needed info in the same place and allows for easy contract creation/interaction
class Address {
    constructor(mnemonic = "") {
        this.terra = new terra_js_1.LCDClient(env_helper_1.env["chain"]);
        const mk = new terra_js_1.MnemonicKey({
            mnemonic: mnemonic
        });
        this.wallet = this.terra.wallet(mk);
    }
    signAndBroadcast(msgs, memo = "") {
        return __awaiter(this, void 0, void 0, function* () {
            const tx = yield this.wallet.createAndSignTx({
                msgs: msgs,
                memo: memo
            });
            return yield this.terra.tx.broadcast(tx);
        });
    }
    getAddress() {
        return this.wallet.key.accAddress;
    }
    getContract(contractAddress) {
        return new Contract(this, contractAddress);
    }
    send(address, coins) {
        return __awaiter(this, void 0, void 0, function* () {
            const send = new terra_js_1.MsgSend(this.wallet.key.accAddress, address, coins);
            return yield this.signAndBroadcast([send], "Pas de memo");
        });
    }
    uploadContract(binaryFile) {
        return __awaiter(this, void 0, void 0, function* () {
            const storeCode = new terra_js_1.MsgStoreCode(this.wallet.key.accAddress, fs.readFileSync(binaryFile).toString('base64'));
            let storeCodeTxResult = yield this.signAndBroadcast([storeCode]);
            if ((0, terra_js_1.isTxError)(storeCodeTxResult)) {
                throw new Error(`store code failed. code: ${storeCodeTxResult.code}, codespace: ${storeCodeTxResult.codespace}, raw_log: ${storeCodeTxResult.raw_log}`);
            }
            const { store_code: { code_id }, } = storeCodeTxResult.logs[0].eventsByType;
            return code_id;
        });
    }
    instantiateContract(codeId, initMsg) {
        return __awaiter(this, void 0, void 0, function* () {
            const instantiate = new terra_js_1.MsgInstantiateContract(this.wallet.key.accAddress, this.wallet.key.accAddress, codeId, // code ID
            initMsg, {});
            const instantiateTxResult = yield this.signAndBroadcast([instantiate]);
            if ((0, terra_js_1.isTxError)(instantiateTxResult)) {
                throw new Error(`instantiate failed. code: ${instantiateTxResult.code}, codespace: ${instantiateTxResult.codespace}, raw_log: ${instantiateTxResult.raw_log}`);
            }
            const { instantiate_contract: { contract_address }, } = instantiateTxResult.logs[0].eventsByType;
            return this.getContract(contract_address[0]);
        });
    }
}
exports.Address = Address;
/// Allows the messages to be called via methods instead of wrapped objects
function createWrapperProxy(wrapper) {
    let handler = {
        get: function (target, prop, receiver) {
            if (!(prop in target))
                return function (args) {
                    return target.execute(prop.toString(), args);
                };
            else
                return Reflect.get(target, prop);
        }
    };
    return new Proxy(wrapper, handler);
}
