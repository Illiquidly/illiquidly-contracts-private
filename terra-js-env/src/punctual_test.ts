import { Address } from "./terra_utils";
import { env } from "./env_helper";


function getContractLog(response: any){
	return response.logs[0].eventsByType.from_contract;
}


async function main(){

	// Getting a handler for the current address
	let handler = new Address(env["mnemonics"][2]);
	let p2p = handler.getContract(env.contracts.p2p);
	let response;
	/*
	response = await p2p.execute.suggest_counter_trade({
		trade_id:17
	});
	console.log(response);
	*/

	response = await p2p.query.get_all_counter_trades( {
        filters: {
        	owner: handler.getAddress()
        }
    })
	console.log(response);




}

main().then(resp => {
  console.log(resp);
}).catch(err => {
  console.log(err);
})
