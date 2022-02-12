import { Address } from "./terra_utils";
import { env } from "./env_helper";

async function main(){

	// Getting a handler for the current address
	let handler = new Address(env["mnemonics"][0]);

	// Uploading the contract code
	let codeId = await handler.uploadContract("../artifacts/iliq_token.wasm");

	// Instantiating the contract
	let initMsg = {
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
	let contract = await handler.instantiateContract(+codeId[0],initMsg);


	// Testing the send function
	let response = await handler.send(handler.getAddress(),{ uluna:"500000", uusd:"500000"})
	console.log(response);

	// Testing the query function
	response = await contract.query.balance({address:handler.getAddress()})
	console.log(response);

	// Testing the execute function
	response = await contract.execute.burn({ amount:"500000"})
	console.log(response);

	// Asserting side effects
	response = await contract.query.balance({address:handler.getAddress()})
	console.log(response);
}

main().then(resp => {
  console.log(resp);
}).catch(err => {
  console.log(err);
})
