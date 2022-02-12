"use strict";
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
const terra_utils_1 = require("./terra_utils");
const env_helper_1 = require("./env_helper");
function main() {
    return __awaiter(this, void 0, void 0, function* () {
        // Getting a handler for the current address
        let handler = new terra_utils_1.Address(env_helper_1.env["mnemonics"][0]);
        // Uploading the contract code
        let codeId = yield handler.uploadContract("../artifacts/iliq_token.wasm");
        // Instantiating the contract
        let initMsg = {
            custom: "tesoutil",
            name: "ILLIQUIDLY TOKEN",
            symbol: "ILIQ",
            decimals: 6,
            initial_balances: [
                {
                    address: handler.getAddress(),
                    amount: "1000000"
                }
            ]
        };
        let contract = yield handler.instantiateContract(+codeId[0], initMsg);
        // Testing the send function
        let response = yield handler.send(handler.getAddress(), { uluna: "500000", uusd: "500000" });
        console.log(response);
        // Testing the query function
        response = yield contract.query.balance({ address: handler.getAddress() });
        console.log(response);
        // Testing the execute function
        response = yield contract.execute.burn({ amount: "500000" });
        console.log(response);
        // Asserting side effects
        response = yield contract.query.balance({ address: handler.getAddress() });
        console.log(response);
    });
}
main().then(resp => {
    console.log(resp);
}).catch(err => {
    console.log(err);
});
