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


function histogram(p, dataset, id) {
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


axios.get("/data/example/")
    .then(function(response) {
        var pie_dataset = get_pie_dataset(response.data);
        var p = new Piechart(pie_dataset, '#chart');

        var histo_dataset = get_histo_dataset(response.data);
        histogram(p, histo_dataset, '#chart');
    })
