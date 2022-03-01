import { Address } from "./terra_utils";
import { env, add_uploaded_token, add_contract } from "./env_helper";

async function main(){

	// Getting a handler for the current address
	let handler = new Address(env["mnemonics"][0]);

	// Uploading the contract code	
	let p2p_codeId: string[] = await handler.uploadContract("../artifacts/p2p_trading.wasm");

	let p2pInitMsg = {
		name: "P2PTrading"
	}

	let p2p = await handler.instantiateContract(+p2p_codeId[0],p2pInitMsg);
	add_contract("p2p",p2p.address);

	return ["p2p", p2p.address];
}

main().then(resp => {
  console.log(resp);
}).catch(err => {
  console.log(err);
})
