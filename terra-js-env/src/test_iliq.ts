import { Address } from "./terra_utils";
import { env } from "./env_helper";

async function main(){

	// Getting a handler for the current address
	let handler = new Address(env["mnemonics"][0]);

	// Uploading the contract code
	let iliq_codeId: string[] = await handler.uploadContract("../artifacts/iliq_token.wasm");

	// Instantiating the contract
	let iliqInitMsg = {
		custom: "tesoutil",
		name:"ILLIQUIDLY TOKEN",
		symbol:"ILIQ",
		decimals:6,
		initial_balances: [
			{
				address:handler.getAddress(),
				amount: "1000000"
			}
		]
	}
	let iliq = await handler.instantiateContract(+iliq_codeId[0],iliqInitMsg);


	// Testing the send function
	let response = await handler.send(handler.getAddress(),{ uluna:"500000", uusd:"500000"})
	console.log(response);

	// Testing the query function
	response = await iliq.query.balance({address:handler.getAddress()})
	console.log(response);

	// Testing the execute function
	response = await iliq.execute.burn({ amount:"500000"})
	console.log(response);

	// Asserting side effects
	response = await iliq.query.balance({address:handler.getAddress()})
	console.log(response);
}

main().then(resp => {
  console.log(resp);
}).catch(err => {
  console.log(err);
})
