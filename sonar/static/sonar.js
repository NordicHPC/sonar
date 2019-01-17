'use strict';


function get_histo_dataset(data) {
    let histo_dataset = [];
    for (let swname in data.software) {
        let swobj = data.software[swname];
        histo_dataset.push({
            name: swname,
            percent: swobj.percent
        });
    }
    return histo_dataset;
}


function get_pie_dataset(data) {
    let pie_dataset = {};
    for (let swname in data.software) {
        let swobj = data.software[swname];
        let subcall_list = [];
        for (let subcall in swobj.subcalls) {
            subcall_list.push({
                name: subcall,
                percent: swobj.subcalls[subcall]
            });
        }
        pie_dataset[swname] = subcall_list;
    }
    return pie_dataset;
}


function color(i) {
    let colors = ["#3366cc", "#dc3912", "#ff9900", "#109618", "#990099", "#0099c6", "#dd4477", "#66aa00", "#b82e2e", "#316395", "#994499", "#22aa99", "#aaaa11", "#6633cc", "#e67300", "#8b0707", "#651067", "#329262", "#5574a6", "#3b3eac"];

    return colors[i % colors.length];
}


function histogram(dataset, id) {
    let num_columns = dataset.length;
    let max_value = Math.ceil(Math.max(...dataset.map(d => d.percent)));

    let dim = {
        width: 600,
        height: 400,
        top: 20,
        left: 30,
        right: 20,
        bottom: 50,
        padding: 0.2
    };

    dim.chartwidth = dim.width - dim.left - dim.right;
    dim.barwidth = dim.chartwidth / num_columns;
    dim.barheight = dim.height - dim.top - dim.bottom;

    let svg = d3.select(id)
        .append('svg')
        .attr('width', dim.width)
        .attr('height', dim.height)
        .attr('class', 'barchart')
        .append('g')
        .attr('transform', 'translate(' + dim.left + ',' + dim.top + ')');

    const barchart = svg.append('g')

    const yScale = d3.scaleLinear()
        .range([dim.barheight, 0])
        .domain([0, max_value]);

    barchart.append('g')
        .call(d3.axisLeft(yScale));

    const xScale = d3.scaleBand()
        .range([0, dim.chartwidth])
        .domain(dataset.map((s) => s.name))
        .padding(dim.padding)

    barchart.append('g')
        .attr('transform', `translate(0, ${dim.barheight})`)
        .call(d3.axisBottom(xScale))
        .selectAll('text')
        .style('text-anchor', 'end')
        .attr('dx', '-.8em')
        .attr('dy', '.15em')
        .attr('transform', 'rotate(-30)');

    let bars = barchart
        .selectAll('.bar')
        .data(dataset)
        .enter()
        .append('g')
        .attr('class', 'bar');

    bars.append('rect')
        .attr('x', (d) => xScale(d.name))
        .attr('y', (d) => yScale(d.percent))
        .attr('height', (s) => dim.barheight - yScale(s.percent))
        .attr('width', xScale.bandwidth())
        .on('mouseover', mouseover)
        .on('mouseout', mouseout);

    bars.append('text').text((d) => d.percent + '%')
        .attr('x', (d) => xScale(d.name) + xScale.bandwidth() / 2)
        .attr('y', (d) => yScale(d.percent) - 5)
        .attr('text-anchor', 'middle');

    function mouseover(d) {
        p.update(d.name);
    }

    function mouseout(d) {
        return;
    }
}

function Piechart(dataset, id, category = 'Vasp') {
    this.dataset = dataset;

    this.dim = {
        width: 300,
        height: 300,
        top: 0,
        left: 0,
        right: 0,
        bottom: 0
    };

    this.dim.chartwidth = this.dim.width - this.dim.left - this.dim.right;
    this.dim.chartheight = this.dim.height - this.dim.top - this.dim.bottom;
    this.dim.radius = Math.min(this.dim.width, this.dim.height) / 2;

    this.svg = d3.select(id)
        .append('svg')
        .attr('width', this.dim.width)
        .attr('height', this.dim.height)
        .attr('class', 'piechart')
        .append('g')
        .attr('transform', 'translate(' + (this.dim.chartwidth / 2 + this.dim.left) + ',' + (this.dim.chartheight / 2 + this.dim.top) + ')');

    this.arc = d3.arc()
        .outerRadius(this.dim.radius - 10)
        .innerRadius(0);

    this.pie = d3.pie().sort(null).value((d) => d.percent);

    this.svg.selectAll('path')
        .data(this.pie(this.dataset[category]))
        .enter()
        .append('path')
        .attr('d', this.arc)
        .each(function(d) {
            this._current = d;
        })
        .style('fill', (d, i) => color(i));

    this.legend = new Legend(this.dataset[category], id);

    this.update = function(category) {
        this.svg.selectAll('path')
            .data(this.pie(dataset[category]))
            .attr('d', this.arc);

        this.svg.selectAll('path')
            .data(this.pie(dataset[category]))
            .enter()
            .append('path')
            .attr('d', this.arc)
            .each(function(d) {
                this._current = d;
            })
            .style('fill', (d, i) => color(i))
            .attr('d', this.arc);

        this.svg.selectAll("path")
            .data(this.pie(dataset[category]))
            .exit()
            .remove();

        this.legend.update(dataset[category]);
    };
}

function Legend(dataset, id) {
    this.update = function(dataset) {
        this.tbody.selectAll("*").remove();

        let rows = this.tbody.selectAll('tr')
            .data(dataset)
            .enter()
            .append('tr');

        // First column (color)
        rows.append('td')
            .append('svg')
            .attr('width', '16')
            .attr('height', '16')
            .append('rect')
            .attr('width', '100%')
            .attr('height', '100%')
            .attr('fill', (d, i) => color(i));

        // Second column (name)
        rows.append('td')
            .text((d) => d.name);

        // Third column (percent)
        rows.append('td')
            .attr('class', 'leg_percent')
            .text((d) => d.percent + '%');
    }

    this.legend = d3.select(id)
        .append('table')
        .attr('class', 'legend');

    this.tbody = this.legend.append('tbody');

    this.update(dataset);
}


const json_data = '{"software": {"Vasp": {"percent": 24.3,"subcalls": {"vasp_std": 43.6,"vasp.5.3.2": 16.8,"std": 15.6,"vasp.NGZhalf": 10.3,"vasp.5.3.5": 9.5,"vasp.gamma": 1.3,"vasp.noncol": 1.2,"std_onlyr_z": 0.6,"vasp_gam": 0.5,"vasp.texas": 0.4,"vasp_std_abfix": 0.1}},"Gaussian": {"percent": 18.7,"subcalls": {"l502.exel": 29.8,"l1002.exel": 18.4,"l502.exe": 10.7,"linda-dummy": 10.5,"l703.exel": 4.8,"l508.exel": 4.4,"l508.exe": 4.1,"l906.exel": 3.6,"l1110.exel": 2.8,"l1002.exe": 2.6,"l703.exe": 2.0,"l1110.exe": 1.4,"l914.exel": 1.2,"l401.exel": 0.6,"l302.exel": 0.5,"l701.exel": 0.5,"l612.exe": 0.3,"cLindaLauncher": 0.3,"l906.exe": 0.3,"l701.exe": 0.2,"l401.exe": 0.2,"l1112.exe": 0.1,"l103.exe": 0.1,"l1101.exel": 0.1,"l811.exe": 0.1,"l914.exe": 0.1,"l302.exe": 0.1,"l101.exe": 0.1}},"OTHER": {"percent": 14.1,"subcalls": {"london.x": 35.0,"gpaw-python": 15.1,"ccsm.exe": 8.4,"cpmd.x": 5.5,"linda-dummy": 4.3,"lmp": 3.5,"eT": 3.1,"python3.6": 3.0,"pmi_proxy": 2.4,"issm.exe": 2.4,"mpiexec.hydra": 2.2,"vina": 1.6,"paradis": 1.3,"mrchem.x": 1.2,"hmmsearch": 0.9,"MATLAB": 0.8,"pw.x": 0.6,"norwecom.e2e.Nordic4_XXL": 0.5,"g16nbo.i8.exe": 0.4,"storm2d": 0.4,"amain.pc_pow_floor": 0.4,"emepctm": 0.3,"gmx_mpi_d": 0.3,"ncks": 0.3,"PEM": 0.3,"orterun": 0.3,"nrniv": 0.3,"ctest": 0.3,"mg41.exe": 0.3,"orted": 0.3,"iopc": 0.2,"OPElectrostatic": 0.2,"discus_suite": 0.2,"sortmerna": 0.2,"leadfinder": 0.2,"run.x": 0.2,"unstable_thermal_conductivity_do_not_use": 0.2,"wega": 0.2,"fortcom": 0.2,"comsollauncher": 0.1,"ifort": 0.1,"jointpdf349-359": 0.1,"cmake": 0.1,"jointpdf1-11": 0.1,"numerical_gradient.exe": 0.1,"bucket.out": 0.1,"norwecom.e2e.rhea": 0.1,"vislcg3": 0.1,"norwecom.e2e.mesopel": 0.1,"ams.exe": 0.1,"xtb": 0.1,"norwecom.e2e.2010_2019": 0.1,"jointpdf170-190": 0.1,"norwecom.e2e.2045_2055": 0.1,"norwecom.e2e.2025_2035": 0.1,"norwecom.e2e.2060_2069": 0.1,"ncl": 0.1}},"ROMS": {"percent": 6.2,"subcalls": {"oceanM": 68.2,"oceanG": 31.8}},"NAMD": {"percent": 3.7,"subcalls": {"namd2": 100.0}},"LAMMPS": {"percent": 3.3,"subcalls": {"lmp_mpi": 99.8,"lmp_mkl": 0.2}},"CP2K": {"percent": 3.2,"subcalls": {"cp2k.popt": 100.0}},"TurboMole": {"percent": 2.7,"subcalls": {"dscf_mpi": 63.9,"ridft_mpi": 11.3,"aoforce_smp": 8.9,"rdgrad_mpi": 6.1,"grad_mpi": 3.1,"dscf_smp": 2.9,"ridft_smp": 1.4,"ricc2_omp": 1.3,"grad_smp": 0.4,"aoforce": 0.3,"rdgrad_smp": 0.2,"mpiexec.hydra": 0.1,"escf_omp": 0.1}},"ADF": {"percent": 2.7,"subcalls": {"adf.exe": 100.0}},"Script": {"percent": 2.5,"subcalls": {"python2.7": 81.8,"bash": 6.0,"java": 5.6,"lua": 2.9,"python": 2.1,"python3.5": 1.2,"tcsh": 0.3,"perl": 0.1,"python3.3": 0.1}},"StagYY": {"percent": 2.4,"subcalls": {"stagyympi": 100.0}},"OpenMPI": {"percent": 2.2,"subcalls": {"orted": 73.3,"orterun": 26.7}},"Schrodinger": {"percent": 2.0,"subcalls": {"desmond": 90.4,"python": 4.9,"perl": 2.9,"glide_backend": 1.7}},"Patmos": {"percent": 1.9,"subcalls": {"xpatmos14_tmp": 64.9,"xpatmos14": 35.0,"xpatmos18_sb": 0.1}},"Qdyn": {"percent": 1.8,"subcalls": {"qdyn5p": 57.5,"Qdyn5p_goodOld": 23.1,"Qdyn5p": 17.9,"Qdyn5p_dm": 0.9,"Qdyn5": 0.7}},"fiber": {"percent": 1.8,"subcalls": {"fiber": 100.0}},"Dalton": {"percent": 1.3,"subcalls": {"lsdalton.x": 57.7,"dalton.x": 42.3}},"KMCART": {"percent": 0.9,"subcalls": {"KMCART_stalloFOSS_kink_v174eaca": 60.1,"KMCART_stalloFOSS_kink3_v174eaca": 15.0,"KMCART_stalloFOSS_test_kink2_v174eaca": 7.8,"KMCART_stalloFOSS_screw_v174eaca": 6.6,"KMCART_stalloFOSS_kink11_v174eaca": 4.7,"KMCART_stalloFOSS_kink1_v174eaca": 2.4,"KMCART_stalloFOSS_kinked_screw_v174eaca": 2.4,"KMCART_stalloFOSS_screw2_v174eaca": 0.5,"KMCART_stallo_kink3_v174eaca": 0.3,"KMCART_stalloFOSS_vbc3a199": 0.2,"KMCART_stalloFOSS_top_kink_v174eaca": 0.1}}}}';

const parsed_data = JSON.parse(json_data);


var histo_dataset = get_histo_dataset(parsed_data);
histogram(histo_dataset, '#chart');

var pie_dataset = get_pie_dataset(parsed_data);
var p = new Piechart(pie_dataset, '#chart');
